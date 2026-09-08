import { localRow, widestLane, type Frame } from './frame';
import { fittedMetrics, graphWidthFor, laneX, type Metrics } from './layout';

/**
 * The lane column, as the commit list beside it has to see it.
 *
 * `layout.ts` answers where a lane is drawn. These three answer what the column costs the row
 * it sits in: how wide it wants to be, how much of it is empty on a given row, and what colour
 * that row's commit came out.
 */

/** The most of the pane the lanes may take before the commit message goes with them. */
const SHARE = 0.35;

/**
 * How wide the lane column wants to be for the rows on screen.
 *
 * Never more than a share of the pane: a merge region thirty lanes wide would otherwise take
 * the commit message with it, and a graph beside no message is not worth the trade. Past this
 * the lanes are drawn tighter instead.
 */
export function columnWidth(
  frame: Frame | null,
  rows: readonly number[],
  paneWidth: number,
  metrics: Metrics,
): number {
  if (rows.length === 0) return 0;
  return Math.min(graphWidthFor(widestLane(frame, rows), metrics), Math.round(paneWidth * SHARE));
}

/**
 * The empty pixels on one row between its outermost lane and the commit message.
 *
 * The column is as wide as the widest row on screen and never narrows again, so on a linear
 * stretch of a repository that has merge regions elsewhere this is most of the column: a
 * corridor of nothing between a commit's node and the text about it. The lane colour fills it,
 * which is what ties the two together.
 *
 * Measured per row rather than for the screen, because the row beside a thirty-lane merge has
 * no gap at all and painting one would cover the lanes.
 */
export function bandWidth(
  frame: Frame | null,
  row: number,
  columnPx: number,
  metrics: Metrics,
  /** The rows on screen, which is what the lanes were fitted to. */
  rows: readonly number[],
): number {
  if (frame === null || localRow(frame, row) === null) return 0;
  const fitted = fittedMetrics(widestLane(frame, rows), columnPx, metrics);
  // To the centre of the outermost node, not past its edge: the band runs under the right half
  // of it, which is what ties the colour to the commit rather than leaving it floating beside
  // one. The canvas is drawn over the top, so the node stays a circle.
  return Math.max(0, columnPx - laneX(widestLane(frame, [row]), fitted));
}

/**
 * The lane a row's commit sits in, as a colour token number.
 *
 * The eight lane colours repeat, so this is the lane modulo eight and matches exactly what the
 * canvas drew for that row.
 */
export function laneToken(frame: Frame | null, row: number): number {
  const local = localRow(frame, row);
  if (local === null || frame === null) return 1;
  return ((frame.lanes[local] ?? 0) % 8) + 1;
}
