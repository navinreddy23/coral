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
  const top = Math.floor(scrollTop / m.rowHeight);
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
