#!/usr/bin/env python3
"""Measures a Git client opening a repository, the same way for every client.

Runs on a nested X server (`Xephyr :9`), so the screen the clock watches holds the client
and nothing else. See README.md beside this file for the method and how to re-run it.
"""

import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

from Xlib import X, display

DISPLAY = ":9"
# Long enough that a splash or a loading screen held still is not taken for the result.
STILL_FOR = 5.0
GIVE_UP_AFTER = 180.0
SETTLE_BEFORE_MEMORY = 20.0
IDLE_WINDOW = 30.0
TICKS = os.sysconf("SC_CLK_TCK")


# Electron reads XDG_SESSION_TYPE, not WAYLAND_DISPLAY, and follows it onto the desktop session.
ELECTRON_ON_X11 = ["--ozone-platform=x11"]


def coral(root: Path, repo: Path, home: Path) -> list[str]:
    # A session.json at the top of the config directory is adopted by the first profile on
    # launch, which opens the repository without driving a file dialog.
    config = home / ".config" / "dev.coral.app"
    config.mkdir(parents=True, exist_ok=True)
    tab = {"id": 1, "path": str(repo), "group": None}
    session = {"tabs": [tab], "groups": [], "active": 1, "nextId": 2}
    (config / "session.json").write_text(json.dumps(session))
    return [str(root / "target" / "release" / "coral-app")]


def gitkraken(root: Path, repo: Path, home: Path) -> list[str]:
    return [str(root / "gitkraken"), *ELECTRON_ON_X11, "--path", str(repo)]


def sublime_merge(root: Path, repo: Path, home: Path) -> list[str]:
    return [str(root / "sublime_merge"), "--new-window", str(repo)]


def github_desktop(root: Path, repo: Path, home: Path) -> list[str]:
    return [str(root / "github-desktop"), *ELECTRON_ON_X11, str(repo)]


CLIENTS = {
    "coral": coral,
    "gitkraken": gitkraken,
    "sublime-merge": sublime_merge,
    "github-desktop": github_desktop,
}


def environment(home: Path) -> dict[str, str]:
    env = dict(os.environ)
    # GTK prefers Wayland whenever WAYLAND_DISPLAY is set, and the window would open on the
    # desktop session while the nested display stays black.
    env.pop("WAYLAND_DISPLAY", None)
    env.update(
        DISPLAY=DISPLAY,
        GDK_BACKEND="x11",
        XDG_SESSION_TYPE="x11",
        HOME=str(home),
        XDG_CONFIG_HOME=str(home / ".config"),
        XDG_DATA_HOME=str(home / ".local" / "share"),
        XDG_CACHE_HOME=str(home / ".cache"),
    )
    return env


def members(root: int, home: Path) -> list[int]:
    # Two ways in, because neither finds everything. Sublime Merge detaches into a session of
    # its own, but keeps the environment it was started with. Electron overwrites its own
    # /proc/<pid>/environ with the process title, but its helpers stay its descendants.
    marker = b"\0HOME=" + bytes(home) + b"\0"
    parents, found = {}, {root}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        try:
            state, parent = (entry / "stat").read_text().rsplit(")", 1)[1].split()[:2]
            if state == "Z":
                continue
            parents[pid] = int(parent)
            environ = b"\0" + (entry / "environ").read_bytes()
        except OSError:
            continue
        if marker in environ:
            found.add(pid)
    grew = True
    while grew:
        grown = found | {pid for pid, parent in parents.items() if parent in found}
        grew, found = grown != found, grown
    return sorted(pid for pid in found if pid in parents)


def pss_kib(pids: list[int]) -> int:
    total = 0
    for pid in pids:
        try:
            rollup = Path(f"/proc/{pid}/smaps_rollup").read_text()
        except OSError:
            continue
        for line in rollup.splitlines():
            if line.startswith("Pss:"):
                total += int(line.split()[1])
    return total


def cpu_ticks(pids: list[int]) -> int:
    total = 0
    for pid in pids:
        try:
            fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        except OSError:
            continue
        total += int(fields[11]) + int(fields[12])
    return total


def frame(screen) -> bytes:
    root = screen.root
    image = root.get_image(0, 0, screen.width_in_pixels, screen.height_in_pixels, X.ZPixmap, 0xFFFFFFFF)
    # Every 97th byte is enough to see a repaint, and keeps one poll well under the interval.
    return bytes(image.data[::97])


def time_to_still(started: float) -> float:
    screen = display.Display(DISPLAY).screen()
    blank = frame(screen)
    last, changed = blank, None
    while time.monotonic() - started < GIVE_UP_AFTER:
        time.sleep(0.05)
        current = frame(screen)
        now = time.monotonic()
        if current != last:
            last, changed = current, now
        elif changed is not None and now - changed >= STILL_FOR:
            return changed - started
    raise TimeoutError("the screen never stopped changing")


def screenshot(path: Path) -> None:
    subprocess.run(["import", "-display", DISPLAY, "-window", "root", str(path)], check=True)


def run_once(client: str, root: Path, repo: Path, template: Path | None, work: Path, n: int) -> dict:
    home = work / f"home-{n}"
    shutil.rmtree(home, ignore_errors=True)
    if template:
        shutil.copytree(template, home, symlinks=True)
    else:
        home.mkdir(parents=True)
    command = CLIENTS[client](root, repo, home)
    started = time.monotonic()
    child = subprocess.Popen(
        command, env=environment(home), cwd=home, start_new_session=True,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        still = time_to_still(started)
        screenshot(work / f"{client}-{n}.png")
        time.sleep(SETTLE_BEFORE_MEMORY)
        running = members(child.pid, home)
        memory = pss_kib(running)
        before = cpu_ticks(running)
        time.sleep(IDLE_WINDOW)
        idle = (cpu_ticks(members(child.pid, home)) - before) / TICKS / IDLE_WINDOW * 100
    finally:
        stop(child, home)
    return {
        "seconds_to_still": round(still, 2),
        "pss_mib": round(memory / 1024),
        "idle_cpu_percent": round(idle, 2),
        "processes": len(running),
    }


def stop(child: subprocess.Popen, home: Path) -> None:
    signal_all(child.pid, home, signal.SIGTERM)
    deadline = time.monotonic() + 30
    while members(child.pid, home) and time.monotonic() < deadline:
        time.sleep(0.5)
    signal_all(child.pid, home, signal.SIGKILL)
    child.wait()


def signal_all(root: int, home: Path, sig: signal.Signals) -> None:
    for pid in members(root, home):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("client", choices=CLIENTS)
    parser.add_argument("root", type=Path, help="where the client is unpacked, or Coral's checkout")
    parser.add_argument("repo", type=Path)
    parser.add_argument("work", type=Path, help="scratch directory for homes and screenshots")
    parser.add_argument("--template", type=Path, help="a home the client's first-run setup was done in")
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()

    args.work.mkdir(parents=True, exist_ok=True)
    runs = [
        run_once(args.client, args.root.resolve(), args.repo.resolve(), args.template, args.work, n)
        for n in range(args.runs)
    ]
    json.dump({"client": args.client, "repo": str(args.repo), "runs": runs}, sys.stdout, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
