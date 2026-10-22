"""Signs in to the fake host from the status bar, photographs the pull requests, and checks
the newest out from its menu.

    drive.py <display> <shots-dir>
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "window-real"))
from driver import Driver  # noqa: E402

DISPLAY, SHOTS = sys.argv[1], Path(sys.argv[2])
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

# The first proposal's row, then the first thing on its menu.
x.click(120, 334, button=3)
x.settle()
x.shot(SHOTS / "proposal-menu.png")
x.click(160, 346)
time.sleep(3)
x.settle(quiet=2.0)
x.shot(SHOTS / "checked-out.png")
sys.exit(1 if x.failures else 0)
