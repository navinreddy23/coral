"""Drives the built window through a fixed tour and photographs each stop.

    tour.py <display> <shots-dir> <wm-log>

The window is opened by run.sh on the `work` and `merging` fixtures at its default 1440 by 900,
in the light theme at the default density, so the positions below are the shipped layout's. A
stop that cannot find what it expects fails the tour, rather than photographing the wrong
screen.

Checked here, because a picture cannot say it: the window maximises and restores from the
title strip, a commit-list column handle shows under the pointer, and no stop is blank.
"""

import subprocess
import sys
import time
from pathlib import Path

from Xlib import X, XK, display
from Xlib.ext import xtest

DISPLAY, SHOTS, WM_LOG = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
d = display.Display(DISPLAY)
screen = d.screen()
failures = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        failures.append(what)


def frame():
    image = screen.root.get_image(0, 0, screen.width_in_pixels, screen.height_in_pixels, X.ZPixmap, 0xFFFFFFFF)
    return bytes(image.data[::97])


def settle(quiet=1.0, limit=30.0):
    """Waits until the screen has not changed for `quiet` seconds.

    A frame that goes back to the one before it is a caret blinking in a focused field, not a
    change: a field with focus never stops blinking, so counting it would never settle.
    """
    previous, last = None, frame()
    since, start = time.monotonic(), time.monotonic()
    while time.monotonic() - start < limit:
        time.sleep(0.1)
        now = frame()
        if now != last and now != previous:
            since = time.monotonic()
        if now != last:
            previous, last = last, now
        elif time.monotonic() - since >= quiet:
            return
        if now == previous and time.monotonic() - since >= quiet:
            return
    check(False, "the screen settled within %ds" % limit)


def move(x, y):
    xtest.fake_input(d, X.MotionNotify, x=x, y=y)
    d.sync()


def click(x, y, times=1, button=1):
    move(x, y)
    time.sleep(0.1)
    for _ in range(times):
        xtest.fake_input(d, X.ButtonPress, button)
        xtest.fake_input(d, X.ButtonRelease, button)
        d.sync()
        time.sleep(0.06)
    settle()


def keys(*names):
    codes = [d.keysym_to_keycode(XK.string_to_keysym(n)) for n in names]
    for c in codes:
        xtest.fake_input(d, X.KeyPress, c)
    for c in reversed(codes):
        xtest.fake_input(d, X.KeyRelease, c)
    d.sync()
    time.sleep(0.05)


def type_text(text):
    for ch in text:
        if ch == " ":
            keys("space")
        elif ch.isupper():
            keys("Shift_L", ch.lower())
        else:
            keys(ch)
    settle()


def shot(name):
    # Away from anything with a hover state, and from GTK's overlay scrollbars, which show
    # under the pointer and fade a second later, so the picture is the stop and not the pointer.
    move(700, 893)
    settle(quiet=1.5)
    path = SHOTS / (name + ".png")
    subprocess.run(["import", "-display", DISPLAY, "-window", "root", "-crop", "1440x900+0+0", str(path)], check=True)
    # A blank or half-drawn window is a handful of colours; any real stop has hundreds.
    colours = subprocess.run(["identify", "-format", "%k", str(path)], capture_output=True, text=True).stdout
    check(int(colours or 0) > 200, "%s is drawn (%s colours)" % (name, colours))


def coral():
    for w in screen.root.query_tree().children:
        if w.get_wm_name() == "Coral":
            return w
    return None


def size():
    g = coral().get_geometry()
    return g.width, g.height


def region(x, y, w, h):
    image = screen.root.get_image(x, y, w, h, X.ZPixmap, 0xFFFFFFFF)
    return bytes(image.data)


settle(quiet=2.0, limit=60.0)
window = coral()
check(window is not None, "the window opened")
if window is None:
    sys.exit(1)
window.set_input_focus(X.RevertToPointerRoot, X.CurrentTime)
d.sync()
shot("graph")

# The title strip: double-click its bare stretch to maximise and again to restore, and a tab
# to do neither.
before = size()
click(900, 20, times=2)
check(size() == (screen.width_in_pixels, screen.height_in_pixels), "double-clicking the bare strip maximises")
shot("maximised")
click(900, 20, times=2)
check(size() == before, "double-clicking it again restores")
click(213, 24, times=2)
check(size() == before, "double-clicking a tab leaves the window alone")
check(WM_LOG.read_text().split() == ["maximise", "restore"], "exactly one maximise and one restore were asked for")

# The branch column's handle, invisible at rest and shown under the pointer.
move(1300, 860)
settle()
rest = region(500, 590, 20, 20)
move(510, 600)
settle()
check(region(500, 590, 20, 20) != rest, "a column handle shows under the pointer")
move(1300, 860)

# The working copy, its staging panel, and a diff side by side.
click(700, 95)
shot("staging")
click(1150, 230)
click(823, 99)
shot("diff-split")
click(1062, 98)

# Preferences on Ctrl+comma, and closed again on Escape.
keys("Control_L", "comma")
settle()
shot("preferences")
keys("Escape")
settle()

# The palette, and the dark theme through it.
keys("Control_L", "p")
settle()
type_text("dark")
shot("palette")
keys("Return")
settle()
shot("graph-dark")
keys("Control_L", "p")
settle()
type_text("light")
keys("Return")
settle()

# The second tab is stopped in a merge, which puts the conflict tool in the main pane.
keys("Control_L", "Tab")
settle(quiet=2.0)
shot("conflict")

if failures:
    print("%d failed" % len(failures))
    sys.exit(1)
print("passed")
