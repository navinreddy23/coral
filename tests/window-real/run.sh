#!/usr/bin/env bash
#
# Drives the built window itself, under WebKitGTK, through a fixed tour.
#
# `just window-test` photographs the frontend in Chrome against the fixture engine, which is
# quick and needs no Rust, but it is not the window people use. Two faults of the 1.2.3 refresh
# showed only here: a file tree painting the wrong ground, and a conflict marker the gutter
# painted over. This runs the release binary on a private X display, on two generated
# repositories, and:
#
# - checks what a picture cannot show, such as the window maximising from its title strip;
# - photographs each stop into target/window-real/shots;
# - compares each photograph with the previous run's and lists the ones that changed, for a
#   person to look at. A change is not a failure, because most are intended.
#
# `run.sh --hold [seconds] [repository...]` sets the same window up and leaves it open
# instead, on the fixtures or on the repositories named.
#
# Needs Xephyr, ImageMagick and python3; python-xlib goes into a venv under target. Nothing
# reaches the desktop the user is working in: the display is a private one, and the window's
# home, config and data directories are thrown away afterwards.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="$root/target/window-real"
binary="$root/target/release/coral-app"

for tool in Xephyr import compare python3; do
    command -v "$tool" >/dev/null || { echo "window-real needs $tool" >&2; exit 1; }
done
[ -x "$binary" ] || { echo "no $binary; run just app first" >&2; exit 1; }

venv="$out/venv"
if [ ! -x "$venv/bin/python" ]; then
    python3 -m venv "$venv"
    "$venv/bin/pip" install --quiet python-xlib
fi

# The first free display from :90, so a second run, or anything else nested, is left alone.
n=90
while [ -e "/tmp/.X11-unix/X$n" ]; do n=$((n + 1)); done
DISPLAY_ID=":$n"

work="$(mktemp -d)"
pids=()
cleanup() {
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
    rm -rf "$work"
}
trap cleanup EXIT

Xephyr "$DISPLAY_ID" -screen 1500x950 -ac -noreset >/dev/null 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

: > "$work/wm.log"
"$venv/bin/python" -I "$here/wm.py" "$DISPLAY_ID" "$work/wm.log" &
pids+=($!)

bash "$here/fixtures.sh" "$work" >/dev/null 2>&1

# A session naming the repositories, written where the window adopts it on first launch: the
# two fixtures, or whatever `--hold` was given after its seconds.
repos=("$work/work" "$work/merging")
[ "${1:-}" = "--hold" ] && [ $# -gt 2 ] && repos=("${@:3}")
home="$work/home"
mkdir -p "$home/.config/dev.coral.app" "$work/bin"
tabs=""
for i in "${!repos[@]}"; do
    tabs+="${tabs:+,}{\"id\":$((i + 1)),\"path\":\"${repos[$i]}\",\"group\":null}"
done
printf '{"tabs":[%s],"groups":[],"active":1,"nextId":%d}' "$tabs" $((${#repos[@]} + 1)) \
    > "$home/.config/dev.coral.app/session.json"
# git is allowed to want ssh only if something is wrong; it gets one that answers nothing.
printf '#!/bin/sh\nexit 255\n' > "$work/bin/ssh"
chmod +x "$work/bin/ssh"

# GTK prefers Wayland whenever WAYLAND_DISPLAY is set, and would open on the user's desktop.
env -u WAYLAND_DISPLAY XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 DISPLAY="$DISPLAY_ID" \
    HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
    XDG_CACHE_HOME="$home/.cache" PATH="$work/bin:$PATH" \
    "$binary" >"$work/app.log" 2>&1 &
pids+=($!)

# `--hold [seconds]` opens the window and leaves it, for driving by hand or by another script,
# instead of touring it.
if [ "${1:-}" = "--hold" ]; then
    echo "holding the window on DISPLAY=$DISPLAY_ID for ${2:-600}s"
    sleep "${2:-600}"
    exit 0
fi

mkdir -p "$out/shots" "$out/last"
rm -f "$out"/shots/*.png
status=0
"$venv/bin/python" -I "$here/tour.py" "$DISPLAY_ID" "$out/shots" "$work/wm.log" || status=$?

# Under a thousand pixels is the working copy's spinner and the like, which never stops; a
# layout change moves tens of thousands.
echo
for shot in "$out"/shots/*.png; do
    name="$(basename "$shot")"
    if [ -f "$out/last/$name" ]; then
        changed="$(compare -metric AE -fuzz 2% "$out/last/$name" "$shot" null: 2>&1 || true)"
        changed="${changed%% *}"
        changed="${changed%%.*}"
        if [ "${changed:-0}" -ge 1000 ] 2>/dev/null; then
            echo "changed since the last run: $name ($changed px)"
        fi
    fi
done
cp "$out"/shots/*.png "$out/last/" 2>/dev/null || true
echo "photographs in $out/shots"
exit "$status"
