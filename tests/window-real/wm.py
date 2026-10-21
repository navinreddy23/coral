"""The least a window manager can be for the tour: it maps windows, gives them the keyboard,
honours configure requests, and maximises and restores on _NET_WM_STATE.

Without one on the nested display nothing answers a maximise and no window has the keyboard
focus, so a typed key goes nowhere and double-clicking the title strip proves nothing.
Every maximise request is written to the log the tour reads back.
"""

import sys

from Xlib import X, Xatom, display

d = display.Display(sys.argv[1])
root = d.screen().root
WIDTH, HEIGHT = d.screen().width_in_pixels, d.screen().height_in_pixels
STATE = d.intern_atom("_NET_WM_STATE")
MAX_V = d.intern_atom("_NET_WM_STATE_MAXIMIZED_VERT")
MAX_H = d.intern_atom("_NET_WM_STATE_MAXIMIZED_HORZ")
log = open(sys.argv[2], "a", buffering=1)

root.change_attributes(event_mask=X.SubstructureRedirectMask | X.SubstructureNotifyMask)
check = root.create_window(0, 0, 1, 1, 0, X.CopyFromParent)
for w in (root, check):
    w.change_property(d.intern_atom("_NET_SUPPORTING_WM_CHECK"), Xatom.WINDOW, 32, [check.id])
root.change_property(d.intern_atom("_NET_SUPPORTED"), Xatom.ATOM, 32, [STATE, MAX_V, MAX_H])
d.sync()

restore_to = {}
while True:
    e = d.next_event()
    if e.type == X.MapRequest:
        e.window.map()
        d.sync()
        e.window.set_input_focus(X.RevertToPointerRoot, X.CurrentTime)
    elif e.type == X.ConfigureRequest:
        e.window.configure(x=e.x, y=e.y, width=e.width, height=e.height)
    elif e.type == X.ClientMessage and e.client_type == STATE:
        action, first, second = e.data[1][0], e.data[1][1], e.data[1][2]
        if MAX_V not in (first, second) and MAX_H not in (first, second):
            continue
        w = e.window
        on = w.id in restore_to
        want = {0: False, 1: True, 2: not on}[action]
        log.write(("maximise" if want else "restore") + "\n")
        if want and not on:
            g = w.get_geometry()
            restore_to[w.id] = (g.x, g.y, g.width, g.height)
            w.configure(x=0, y=0, width=WIDTH, height=HEIGHT)
            w.change_property(STATE, Xatom.ATOM, 32, [MAX_V, MAX_H])
        elif not want and on:
            x, y, width, height = restore_to.pop(w.id)
            w.configure(x=x, y=y, width=width, height=height)
            w.change_property(STATE, Xatom.ATOM, 32, [])
    d.flush()
