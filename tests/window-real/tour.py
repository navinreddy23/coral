"""Drives the built window through a fixed tour and photographs each stop.

    tour.py <display> <shots-dir> <wm-log>

The window is opened by run.sh on the `work` and `merging` fixtures at its default 1440 by 900,
in the light theme at the default density, so the positions below are the shipped layout's. A
stop that cannot find what it expects fails the tour, rather than photographing the wrong
screen.

Checked here, because a picture cannot say it: the window maximises and restores from the
title strip, a commit-list column handle shows under the pointer, and no stop is blank.
"""

import sys
from pathlib import Path

# -I leaves the script's own directory off the path, and the driver lives beside it.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from driver import Driver  # noqa: E402

DISPLAY, SHOTS, WM_LOG = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
x = Driver(DISPLAY)
check, settle, move, click, keys, type_text = x.check, x.settle, x.move, x.click, x.keys, x.type_text
size, region, screen, failures = x.size, x.region, x.screen, x.failures


def shot(name):
    x.shot(SHOTS / (name + ".png"))


settle(quiet=2.0, limit=60.0)
window = x.coral()
check(window is not None, "the window opened")
if window is None:
    sys.exit(1)
x.focus(window)
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
