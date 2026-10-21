"""Asks a running Coral to check for an update and install it, through the palette.

    install.py <display> <shots-dir> refused
    install.py <display> <shots-dir> installed <new-binary>

`refused` expects the install to fail and the window to stay; `installed` expects the window
to go and come back as another process, running <new-binary>.
"""

import sys
import time
from pathlib import Path

from Xlib import X

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "window-real"))
from driver import Driver  # noqa: E402

DISPLAY, SHOTS, MODE = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
x = Driver(DISPLAY)


def pid_of(window):
    prop = window.get_full_property(x.d.intern_atom("_NET_WM_PID"), X.AnyPropertyType)
    return prop.value[0] if prop else None


def palette(query):
    x.keys("Control_L", "p")
    x.settle()
    x.type_text(query)
    x.keys("Return")


def opened():
    window = x.wait_for_coral()
    if not x.check(window is not None, "the window opened"):
        sys.exit(1)
    x.settle(quiet=2.0, limit=60.0)
    x.focus(window)
    return window


window = opened()
before = pid_of(window)

palette("check for updates")
x.settle(quiet=2.0)
x.shot(SHOTS / (MODE + "-found.png"))
x.keys("Escape")
x.settle()

palette("install coral")
if MODE == "refused":
    # The download is the size of the application; give it time to arrive and be checked.
    time.sleep(8)
    x.settle(quiet=2.0)
    x.shot(SHOTS / "refused.png")
    window = x.coral()
    x.check(window is not None and pid_of(window) == before, "the window stays after a refused update")
else:
    expected = Path(sys.argv[4]).read_bytes()
    start = time.monotonic()
    while time.monotonic() - start < 120:
        window = x.coral()
        if window is not None and pid_of(window) not in (None, before):
            break
        time.sleep(0.5)
    else:
        window = None
    x.check(window is not None, "a new window came back after installing")
    if window is not None:
        x.settle(quiet=2.0, limit=60.0)
        x.focus(window)
        exe = Path("/proc/%d/exe" % pid_of(window)).read_bytes()
        x.check(exe == expected, "the new window is running the newer binary")
        palette("check for updates")
        x.settle(quiet=2.0)
        x.shot(SHOTS / "installed.png")

if x.failures:
    print("%d failed" % len(x.failures))
    sys.exit(1)
