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
 * How far the row's lane colour reaches back into the lane column: from its commit's node to
 * the commit message.
 *
 * The band runs behind every lane line to the right of the node, which the canvas draws over
 * it, so a branch reads as one ribbon of colour from its node to its text.
 */
export function bandWidth(
  frame: Frame | null,
  row: number,
  columnPx: number,
  metrics: Metrics,
  /** The rows on screen, which is what the lanes were fitted to. */
  rows: readonly number[],
): number {
  const local = localRow(frame, row);
  if (frame === null || local === null) return 0;
  const fitted = fittedMetrics(widestLane(frame, rows), columnPx, metrics);
  // From the centre of the row's own node, as the reference client draws it. Measured from the
  // outermost lane crossing the row instead, the band began after whichever line ran furthest
  // right, so its left edge jumped from row to row and looked cut off. The canvas is drawn
  // over the top, so the node stays a circle and the lines crossing the band stay on it.
  return Math.max(0, columnPx - laneX(frame.lanes[local] ?? 0, fitted));
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
