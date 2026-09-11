/**
 * Where a menu and its submenus sit so that all of them stay inside the window.
 *
 * Arithmetic rather than a stylesheet, because it depends on measurements only the browser
 * has, and a module rather than two copies inside the component, because the panel and the
 * submenu that hangs off it were getting different answers to the same question.
 */

/** A box, in the coordinates `getBoundingClientRect` uses. */
export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** How much room the window leaves, and how far off its edges anything must stay. */
export interface Viewport {
  width: number;
  height: number;
  margin: number;
}

/** Where a panel opened at a point goes, given how big it turned out to be. */
export function fitPanel(
  x: number,
  y: number,
  size: { width: number; height: number },
  view: Viewport,
): { left: number; top: number } {
  return {
    left: Math.max(view.margin, Math.min(x, view.width - size.width - view.margin)),
    top: Math.max(view.margin, Math.min(y, view.height - size.height - view.margin)),
  };
}

/**
 * Where a submenu goes relative to the row it hangs off.
 *
 * `left` and `top` are CSS lengths against the row, which is what the stylesheet positions it
 * from. A submenu opens to the right of its row and drops from its top, and both of those run
 * out of window: a menu already fitted against the right edge has no room to its right, and
 * the rows of the submenu were simply cut off by the window with no way to read or reach them.
 */
export function fitSubmenu(
  row: Box,
  size: { width: number; height: number },
  view: Viewport,
  /** How far above the row the submenu starts, which lines its first row up with this one. */
  lift = 5,
): { left: string; top: string } {
  const rightward = row.right - 2 + size.width <= view.width - view.margin;
  const spill = row.top - lift + size.height - (view.height - view.margin);
  return {
    // Flipped to the left of the row rather than merely clamped: a submenu shoved back over
    // its own parent covers the labels it was opened from.
    left: rightward ? 'calc(100% - 2px)' : `${2 - size.width}px`,
    top: spill > 0 ? `${-lift - spill}px` : `${-lift}px`,
  };
}
