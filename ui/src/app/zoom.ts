/**
 * The scales the window can be zoomed to.
 *
 * Steps rather than any number: each is a whole or near-whole number of device pixels for the
 * row heights, so the rows stay on the grid the list is pinned to, and Ctrl+= and Ctrl+- land
 * on the same few every time rather than drifting by tenths.
 */
export const ZOOMS = [0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2] as const;

/** The next step up or down from `at`, staying at either end. */
export function stepZoom(at: number, direction: 1 | -1): number {
  const index = ZOOMS.findIndex((z) => Math.abs(z - at) < 0.001);
  const from = index === -1 ? ZOOMS.indexOf(1) : index;
  const to = Math.min(ZOOMS.length - 1, Math.max(0, from + direction));
  return ZOOMS[to] ?? 1;
}

/** The bounds the terminal's text can be set to, in pixels. */
export const TERMINAL_FONT = { min: 9, max: 24, usual: 12 } as const;
