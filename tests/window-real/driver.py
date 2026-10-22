"""Drives a window on a private X display: the pointer, the keyboard, and photographs.

Shared by the tour of the window and by the update test, which run the built binary under
Xephyr. A check that fails is recorded and printed, not raised, so one run reports every
failure rather than the first.
"""

import subprocess
import time

from Xlib import X, XK, display
from Xlib.ext import xtest


# Keysyms by name, since a character is not one: ":" is Shift and the semicolon key on a US
# layout, which is what Xephyr starts with.
PUNCTUATION = {
    " ": ("space",),
    ":": ("Shift_L", "semicolon"),
    "_": ("Shift_L", "minus"),
    "-": ("minus",),
    ".": ("period",),
    "/": ("slash",),
    '"': ("Shift_L", "apostrophe"),
}


class Driver:
    def __init__(self, name):
        self.name = name
        self.d = display.Display(name)
        self.screen = self.d.screen()
        self.failures = []

    def check(self, ok, what):
        print(("ok   " if ok else "FAIL ") + what, flush=True)
        if not ok:
            self.failures.append(what)
        return ok

    def frame(self):
        s = self.screen
        image = s.root.get_image(0, 0, s.width_in_pixels, s.height_in_pixels, X.ZPixmap, 0xFFFFFFFF)
        return bytes(image.data[::97])

    def settle(self, quiet=1.0, limit=30.0):
        """Waits until the screen has not changed for `quiet` seconds.

        A frame that goes back to the one before it is a caret blinking in a focused field,
        not a change: a field with focus never stops blinking, so counting it would never
        settle.
        """
        previous, last = None, self.frame()
        since, start = time.monotonic(), time.monotonic()
        while time.monotonic() - start < limit:
            time.sleep(0.1)
            now = self.frame()
            if now != last and now != previous:
                since = time.monotonic()
            if now != last:
                previous, last = last, now
            elif time.monotonic() - since >= quiet:
                return
            if now == previous and time.monotonic() - since >= quiet:
                return
        self.check(False, "the screen settled within %ds" % limit)

    def move(self, x, y):
        xtest.fake_input(self.d, X.MotionNotify, x=x, y=y)
        self.d.sync()

    def click(self, x, y, times=1, button=1):
        self.move(x, y)
        time.sleep(0.1)
        for _ in range(times):
            xtest.fake_input(self.d, X.ButtonPress, button)
            xtest.fake_input(self.d, X.ButtonRelease, button)
            self.d.sync()
            time.sleep(0.06)
        self.settle()

    def keys(self, *names):
        codes = [self.d.keysym_to_keycode(XK.string_to_keysym(n)) for n in names]
        for c in codes:
            xtest.fake_input(self.d, X.KeyPress, c)
        for c in reversed(codes):
            xtest.fake_input(self.d, X.KeyRelease, c)
        self.d.sync()
        time.sleep(0.05)

    def type_text(self, text):
        for ch in text:
            if ch in PUNCTUATION:
                self.keys(*PUNCTUATION[ch])
            elif ch.isupper():
                self.keys("Shift_L", ch.lower())
            else:
                self.keys(ch)
        self.settle()

    def shot(self, path):
        # Away from anything with a hover state, and from GTK's overlay scrollbars, which show
        # under the pointer and fade a second later, so the picture is the stop and not the
        # pointer.
        self.move(700, 893)
        self.settle(quiet=1.5)
        subprocess.run(["import", "-display", self.name, "-window", "root", "-crop", "1440x900+0+0", str(path)], check=True)
        # A blank or half-drawn window is a handful of colours; any real stop has hundreds.
        colours = subprocess.run(["identify", "-format", "%k", str(path)], capture_output=True, text=True).stdout
        self.check(int(colours or 0) > 200, "%s is drawn (%s colours)" % (path.stem, colours))

    def coral(self):
        for w in self.screen.root.query_tree().children:
            if w.get_wm_name() == "Coral":
                return w
        return None

    def wait_for_coral(self, limit=60.0):
        start = time.monotonic()
        while time.monotonic() - start < limit:
            window = self.coral()
            if window is not None:
                return window
            time.sleep(0.2)
        return None

    def focus(self, window):
        window.set_input_focus(X.RevertToPointerRoot, X.CurrentTime)
        self.d.sync()

    def size(self):
        g = self.coral().get_geometry()
        return g.width, g.height

    def region(self, x, y, w, h):
        image = self.screen.root.get_image(x, y, w, h, X.ZPixmap, 0xFFFFFFFF)
        return bytes(image.data)
