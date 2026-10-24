"""Signs in to the fake host from the status bar, photographs the pull requests, opens one from
a branch the host does not have yet, reads and answers #12's review, and checks #12 out from
its menu.

    drive.py <display> <shots-dir> <github|gitlab>
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "window-real"))
from driver import Driver  # noqa: E402

DISPLAY, SHOTS = sys.argv[1], Path(sys.argv[2])
PROVIDER = sys.argv[3] if len(sys.argv) > 3 else "github"
x = Driver(DISPLAY)
window = x.wait_for_coral(60)
if not x.check(window is not None, "the window opened"):
    sys.exit(1)
x.settle(quiet=2.0, limit=60.0)
x.focus(window)
x.shot(SHOTS / "signed-out.png")

# "GitHub · no token" in the status bar opens the account panel with its field focused.
x.click(1185, 888)
x.settle()
x.shot(SHOTS / "sign-in.png")
x.type_text("dummytoken")
x.keys("Return")
time.sleep(3)
x.settle(quiet=2.0)
x.shot(SHOTS / "checks.png")

# A request from `topic`, which is checked out and on no remote: the form has to push it
# before the host will take it. Fetched first, for the branches it could land on.
x.click(362, 60)
time.sleep(2)
x.settle(quiet=2.0)
x.click(56, 201, button=3)
x.settle()
x.click(175, 257)
x.settle()
x.click(306, 266)
x.keys("Control_L", "a")
x.type_text("Add a note for builders")
x.shot(SHOTS / "proposing.png")
x.click(138, 536)
time.sleep(4)
x.settle(quiet=2.0)
x.shot(SHOTS / "proposed.png")

# #12, now second in the list under the request just opened. The review before the checkout:
# checking out adds a branch above the list and moves it. GitHub's fake review has three
# remarks and GitLab's one, which is where the box lands.
x.click(120, 376, button=3)
x.settle()
x.click(205, 424)
time.sleep(2)
x.settle(quiet=2.0)
box = 373 if PROVIDER == "github" else 281
x.click(345, box)
x.type_text("Added a test")
x.click(70, box + 65)
time.sleep(2)
x.settle(quiet=2.0)
x.shot(SHOTS / "review.png")
x.click(168, box + 65 + 46)
time.sleep(2)
x.settle(quiet=2.0)
x.shot(SHOTS / "approved.png")
x.keys("Escape")
x.settle()

# #12's row, then the first thing on its menu.
x.click(120, 376, button=3)
x.settle()
x.shot(SHOTS / "proposal-menu.png")
x.click(160, 388)
time.sleep(3)
x.settle(quiet=2.0)
x.shot(SHOTS / "checked-out.png")
sys.exit(1 if x.failures else 0)
