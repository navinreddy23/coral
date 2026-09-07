/**
 * Geometry for the commit graph. Pure arithmetic, so it is unit-testable without a canvas.
 */

export interface Metrics {
  rowHeight: number;
  laneWidth: number;
  /** Distance from the left edge to the centre of lane 0. */
  laneOrigin: number;
  /** Outer radius of a commit's ring, stroke included. */
  nodeRadius: number;
}

export const DEFAULT_METRICS: Metrics = {
  rowHeight: 28,
  laneWidth: 20,
  laneOrigin: 15,
  nodeRadius: 10,
};

/**
 * Width of the branch and tag column, which sits left of the lanes.
 *
 * Wide enough for a real branch name. At 190 every name of the shape
 * `bugfix/REAN2-6135/fix-gitlab-pages-deploy-limit` came out as `…/fix-gitlab-pages-…`, which
 * is the part every branch on that ticket shares. Not wider than this: the column is empty on
 * almost every row, and what it takes comes out of the commit message.
 */
export const REFS_COLUMN_PX = 240;

/** Width of the lane column. Wider graphs scroll within it rather than pushing the message. */
export const GRAPH_COLUMN_PX = 170;

/**
 * Narrowest a lane may be drawn.
 *
 * Below this the nodes touch and the lines are indistinguishable, so a graph needing more
 * lanes than fit is clipped rather than squeezed further.
 */
const MIN_LANE_WIDTH = 7;

/**
 * The metrics for drawing `maxLane` lanes in `columnPx` of column.
 *
 * The shipped pitch where it fits, tighter where it does not. A kernel merge region is thirty
 * lanes wide and would need six hundred pixels at the full pitch — more than the column can
 * have without taking the commit message with it — and drawn at the full pitch anyway every
 * node past the edge is simply not on screen, which reads as a graph with no commits in it.
 */
export function fittedMetrics(maxLane: number, columnPx: number): Metrics {
  const base = DEFAULT_METRICS;
  if (maxLane <= 0) return base;

  const room = columnPx - base.laneOrigin - base.nodeRadius - 10;
  const pitch = Math.max(MIN_LANE_WIDTH, Math.floor(room / maxLane));
  if (pitch >= base.laneWidth) return base;

  const nodeRadius = Math.max(3, Math.min(base.nodeRadius, Math.floor(pitch / 2)));
  return { ...base, laneWidth: pitch, nodeRadius, laneOrigin: nodeRadius + 2 };
}

export function laneX(lane: number, m: Metrics): number {
  return m.laneOrigin + lane * m.laneWidth;
}

/**
 * How wide the lane column has to be to hold lanes up to `maxLane`.
 *
 * The outermost node's own radius, plus a gap so it is not flush against the commit message
 * beside it.
 */
export function graphWidthFor(maxLane: number, m: Metrics): number {
  return laneX(Math.max(0, maxLane), m) + m.nodeRadius + 10;
}

export function rowY(row: number, first: number, m: Metrics): number {
  return (row - first) * m.rowHeight + m.rowHeight / 2;
}

/**
 * The tallest scrollable element we will create.
 *
 * WebKit clamps or rescales a layer past roughly 2^25 px, which makes everything inside it
 * blurry and misaligned. The kernel at 1,481,528 rows would need 41.5M px, so past this the
 * scroll position is mapped onto the row range instead of standing for it directly.
 *
 * Set well below the limit rather than just under it: engines rasterise very tall layers at
 * reduced resolution, and since the mapping is proportional a smaller scrollable area costs
 * nothing but a coarser scrollbar. At this height one pixel of the kernel's scrollbar is
 * roughly three quarters of a row.
 */
export const MAX_SPACER_PX = 2_000_000;

/**
 * Height of the sticky column header, which sits inside the scroller and over the rows.
 *
 * It is in the scroller's flow but pinned to the top, so it takes this much off the height
 * available to rows. Counting the whole scroller as row space made the list one row taller
 * than it is: at the bottom of the kernel's 1,481,530 the last row — its very first commit —
 * sat below the fold and could not be scrolled to.
 */
export const COLUMN_HEADER_PX = 26;

/**
 * How tall the scrollable area should be for a graph of `totalRows`.
 *
 * The rows are not scrolled through: the list is pinned under the sticky header and redrawn a
 * whole row at a time, so the scroll range has to cover a whole number of rows *plus* whatever
 * the last screenful leaves over. Without that remainder — up to one row short of it — the
 * final rows sat below the fold with nowhere left to scroll, which on a repository of
 * thirty-six commits meant the last two could not be reached by the wheel at all.
 *
 * `viewportHeight` of zero is the answer before the pane has been measured, and gives the
 * exact-rows height it always gave.
 */
export function spacerHeight(totalRows: number, m: Metrics, viewportHeight = 0): number {
  const exact = totalRows * m.rowHeight;
  if (exact > MAX_SPACER_PX) return MAX_SPACER_PX;
  const usable = Math.max(0, viewportHeight - COLUMN_HEADER_PX);
  return exact + (usable % m.rowHeight);
}

/**
 * The furthest the commit list can be scrolled.
 *
 * The scroller holds the column header as well as the rows, so the range is taller than the
 * spacer by the header. Reading it off the DOM would be the same number; computing it keeps
 * `scrollToRow` and the wheel agreeing with what the list will actually draw.
 */
export function maxScroll(totalRows: number, m: Metrics, viewportHeight: number): number {
  const content = COLUMN_HEADER_PX + spacerHeight(totalRows, m, viewportHeight);
  return Math.max(0, content - viewportHeight);
}

/** How many whole rows a scroller of `viewportHeight` can show at once. */
export function rowsPerScreen(viewportHeight: number, m: Metrics): number {
  return Math.max(1, Math.floor((viewportHeight - COLUMN_HEADER_PX) / m.rowHeight));
}

/** True when the graph is too tall to scroll one pixel per pixel. */
export function isCompressed(totalRows: number, m: Metrics): boolean {
  return totalRows * m.rowHeight > MAX_SPACER_PX;
}

/**
 * The first row to draw for a scroll position.
 *
 * Below the cap this is the obvious division. Above it the scrollbar covers the whole row
 * range in fewer pixels, so a row is a fraction of a pixel and the mapping is proportional.
 */
export function firstRowFor(
  scrollTop: number,
  viewportHeight: number,
  totalRows: number,
  m: Metrics,
): number {
  if (totalRows <= 0) return 0;
  if (!isCompressed(totalRows, m)) return Math.floor(scrollTop / m.rowHeight);

  const maxScroll = Math.max(1, MAX_SPACER_PX - viewportHeight);
  const lastTop = Math.max(0, totalRows - rowsPerScreen(viewportHeight, m));
  const fraction = Math.min(1, Math.max(0, scrollTop / maxScroll));
  return Math.round(fraction * lastTop);
}

/**
 * Where the row list sits inside the scroller.
 *
 * Exactly `scrollTop`, and deliberately not rounded. The list is absolutely positioned in the
 * scrolling content, so what reaches the screen is `top - scrollTop`: pinning it to the raw
 * offset puts it at exactly zero, and every row below it on a whole multiple of the row
 * height. Rounding here is what *causes* blurred text — a scroll container reports a
 * fractional `scrollTop` under trackpad and fractional display scaling, and `round(scrollTop)
 * - scrollTop` then lands the whole list, text and all, up to half a pixel off the grid.
 *
 * Which rows to draw comes from `firstRowFor`, so nothing here changes what is on screen.
 */
export function listTop(scrollTop: number): number {
  return scrollTop;
}

/** Which rows are visible, plus a screen of overscan on each side. */
export interface Window {
  first: number;
  last: number;
}

export function visibleRows(
  scrollTop: number,
  viewportHeight: number,
  totalRows: number,
  m: Metrics,
): Window {
  if (totalRows <= 0) return { first: 0, last: -1 };

  const perScreen = Math.ceil(viewportHeight / m.rowHeight);
  const top = firstRowFor(scrollTop, viewportHeight, totalRows, m);
  const last = Math.min(totalRows - 1, top + perScreen * 2);
  // `first` is clamped against `last`, not only against zero: the graph can shrink while the
  // view is scrolled down — hiding a branch, or swapping in a smaller first-paint store — and
  // an inverted window would silently draw nothing.
  return { first: Math.max(0, Math.min(top - perScreen, last)), last };
}

/** The eight lane colours from tokens.css, resolved once rather than per frame. */
export function laneColours(root: HTMLElement): string[] {
  return palette(root, 'lane');
}

/** The eight node fills, which are what the white initials inside a node sit on. */
export function nodeColours(root: HTMLElement): string[] {
  return palette(root, 'node');
}

function palette(root: HTMLElement, name: string): string[] {
  const style = getComputedStyle(root);
  const read = (n: number) => style.getPropertyValue(`--${name}-${n}`).trim();
  const colours = [1, 2, 3, 4, 5, 6, 7, 8].map(read).filter((c) => c.length > 0);
  return colours.length > 0 ? colours : ['#3fa9f5'];
}

export function laneColour(lane: number, colours: string[]): string {
  return colours[lane % colours.length] ?? '#3fa9f5';
}
