"""Signs in to the fake host from the status bar, photographs the pull requests, reads and
answers the newest one's review, and checks it out from its menu.

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

# The review, before the checkout: checking out adds a branch above the list and moves it.
# GitHub's fake review has three remarks and GitLab's one, which is where the box lands.
x.click(120, 334, button=3)
x.settle()
x.click(205, 382)
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

# The first proposal's row, then the first thing on its menu.
x.click(120, 334, button=3)
x.settle()
x.shot(SHOTS / "proposal-menu.png")
x.click(160, 346)
time.sleep(3)
x.settle(quiet=2.0)
x.shot(SHOTS / "checked-out.png")
sys.exit(1 if x.failures else 0)
