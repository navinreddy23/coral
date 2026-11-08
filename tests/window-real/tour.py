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

# The panel's sections. Each step is undone before the next, and checked back to the picture
# it started from, so the stops after this one find the panel as shipped.
LISTS = (1110, 165, 300, 450)


def same(a, b):
    """Whether two pictures of the lists are the same panel. Not byte for byte: GTK's overlay
    scrollbar fades in and out on its own, which is a handful of pixels and not a layout."""
    return len(a) == len(b) and sum(1 for p, q in zip(a, b) if p != q) < len(a) // 200


move(700, 893)
settle()
shipped = region(*LISTS)
click(1160, 149)
move(700, 893)
settle()
check(not same(region(*LISTS), shipped), "the unstaged list folds to its heading")
shot("sections-folded")
click(1160, 149)
move(700, 893)
settle()
check(same(region(*LISTS), shipped), "and unfolds to the panel it was")
x.drag(1252, 368, 1252, 280)
move(700, 893)
settle()
check(not same(region(*LISTS), shipped), "the divider between the lists follows a drag")
shot("sections-dragged")
click(1252, 280, times=2)
move(700, 893)
settle()
check(same(region(*LISTS), shipped), "and a double-click puts it back")

# A file in the list opens its diff in place of the graph, and the diff goes side by side.
# Checked rather than assumed: a click a few pixels below the file row used to photograph the
# graph under this name, and the tour passed.
HEADER = (270, 86, 780, 24)
graph_top = region(*HEADER)
click(1150, 246)
move(700, 893)
settle()
inline = region(*HEADER)
check(inline != graph_top, "a file in the list opens its diff")
click(798, 99)
move(700, 893)
settle()
check(region(*HEADER) != inline, "the diff goes side by side")
shot("diff-split")
click(1042, 98)
move(700, 893)
settle()


def colour(px, py):
    b, g, r = region(px, py, 1, 1)[:3]
    return (r, g, b)


WHITE = (255, 255, 255)

# Four lines of page part each commit row from the next, two at the foot of one band and two at
# the head of the one under it, and the lane lines run on through the gap rather than stopping
# at it.
# Rows are 28 pixels from the list's top at 81, the working copy's row first; ten commits
# follow it on this fixture.
feet = [108 + 28 * i for i in range(1, 11)]
check(all(colour(600, y + d) == WHITE for y in feet for d in (-1, 0, 1, 2)),
      "every commit row is parted from the next by a gap")
check(all(colour(600, y - 2) != WHITE and colour(600, y + 3) != WHITE for y in feet),
      "with the bands right up to it on either side")
check(all(colour(525, y) != WHITE and colour(525, y + 1) != WHITE for y in feet),
      "and the first lane runs through every one")

# A commit's own panel: the message boxed in the interface face, its author under it.
click(700, 152)
shot("commit-details")

# The branch panel, its tags opened and folded again.
move(700, 893)
settle()
TAGS = (16, 375, 220, 40)
folded = region(*TAGS)
click(80, 363)
move(700, 893)
settle()
check(region(*TAGS) != folded, "the tags open under their heading")
shot("sidebar")
click(80, 363)
move(700, 893)
settle()
check(region(*TAGS) == folded, "and fold away again")

# The find box searching what commits changed, then refusing a date it cannot read. NO_LANE
# was added by one commit and appears in no message.
keys("Control_L", "f")
settle()
type_text("code:NO_LANE")
settle(quiet=2.0)
shot("find-code")
keys("Control_L", "a")
type_text("after:soon")
settle(quiet=2.0)
shot("find-refused")
keys("Escape")
settle()

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
