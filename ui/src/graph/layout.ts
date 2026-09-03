/**
 * Geometry for the commit graph. Pure arithmetic, so it is unit-testable without a canvas.
 */

export interface Metrics {
  rowHeight: number;
  laneWidth: number;
  /** Distance from the left edge to the centre of lane 0. */
  laneOrigin: number;
  nodeRadius: number;
}

export const DEFAULT_METRICS: Metrics = {
  rowHeight: 28,
  laneWidth: 14,
  laneOrigin: 12,
  nodeRadius: 4,
};

export function laneX(lane: number, m: Metrics): number {
  return m.laneOrigin + lane * m.laneWidth;
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
 */
export const MAX_SPACER_PX = 20_000_000;

/** How tall the scrollable area should be for a graph of `totalRows`. */
export function spacerHeight(totalRows: number, m: Metrics): number {
  return Math.min(totalRows * m.rowHeight, MAX_SPACER_PX);
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
  const lastTop = Math.max(0, totalRows - Math.floor(viewportHeight / m.rowHeight));
  const fraction = Math.min(1, Math.max(0, scrollTop / maxScroll));
  return Math.round(fraction * lastTop);
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
  const style = getComputedStyle(root);
  const read = (n: number) => style.getPropertyValue(`--lane-${n}`).trim();
  const colours = [1, 2, 3, 4, 5, 6, 7, 8].map(read).filter((c) => c.length > 0);
  return colours.length > 0 ? colours : ['#3fa9f5'];
}

export function laneColour(lane: number, colours: string[]): string {
  return colours[lane % colours.length] ?? '#3fa9f5';
}
