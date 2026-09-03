import { hasFlag, parentLanesOf, RowFlag, type Frame } from './frame';
import { laneColour, laneX, rowY, type Metrics, type Window } from './layout';

/**
 * Draws the lane column onto a canvas.
 *
 * Deliberately avoided, because each drops WebKitGTK off its accelerated path or defeats its
 * damage tracking: shadows, `ctx.filter`, any composite operation but source-over, gradients
 * or patterns allocated per frame, and `getImageData`. Hit testing is arithmetic on the row
 * model instead, which is also exact.
 */
export function drawLanes(
  ctx: CanvasRenderingContext2D,
  frame: Frame,
  window: Window,
  metrics: Metrics,
  colours: string[],
  width: number,
  height: number,
): void {
  ctx.clearRect(0, 0, width, height);
  ctx.lineWidth = 1.5;
  ctx.lineCap = 'round';

  const first = window.first;
  const rowOf = (absolute: number) => absolute - frame.startRow;

  // Edges first, so nodes sit on top of the curves rather than being cut by them.
  for (let row = window.first; row <= window.last; row++) {
    const local = rowOf(row);
    if (local < 0 || local >= frame.rowCount) continue;

    const lane = frame.lanes[local];
    if (lane === undefined) continue;
    const x0 = laneX(lane, metrics);
    const y0 = rowY(row, first, metrics);

    for (const parentLane of parentLanesOf(frame, local)) {
      const x1 = laneX(parentLane, metrics);
      const y1 = rowY(row + 1, first, metrics);
      ctx.strokeStyle = laneColour(parentLane, colours);
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      if (x0 === x1) {
        // A straight run continues to the bottom of the viewport; the vertical pass below
        // covers the rest of it.
        ctx.lineTo(x1, y1);
      } else {
        // An immediate join: leave this lane at this row and arrive in the parent's by the
        // next, then run straight down.
        const midY = (y0 + y1) / 2;
        ctx.bezierCurveTo(x0, midY, x1, midY, x1, y1);
      }
      ctx.stroke();
    }
  }

  // Vertical runs for lanes that pass through a row without a node in it.
  drawThroughLanes(ctx, frame, window, metrics, colours, first);

  for (let row = window.first; row <= window.last; row++) {
    const local = rowOf(row);
    if (local < 0 || local >= frame.rowCount) continue;

    const lane = frame.lanes[local];
    const flags = frame.rowFlags[local];
    if (lane === undefined || flags === undefined) continue;

    const x = laneX(lane, metrics);
    const y = rowY(row, first, metrics);
    ctx.fillStyle = laneColour(lane, colours);
    ctx.beginPath();
    ctx.arc(x, y, hasFlag(flags, RowFlag.Merge) ? metrics.nodeRadius + 1 : metrics.nodeRadius, 0, Math.PI * 2);
    ctx.fill();
  }
}

/**
 * A lane reserved for a parent stays occupied from the child's row until the parent's own, and
 * exactly one commit is drawn in it over that span. So a vertical run ends at precisely the
 * row whose own lane matches, which the client derives by scanning forward — which is why the
 * frame carries no "lanes ending here" list.
 */
function drawThroughLanes(
  ctx: CanvasRenderingContext2D,
  frame: Frame,
  window: Window,
  metrics: Metrics,
  colours: string[],
  first: number,
): void {
  const open = new Map<number, number>(); // lane -> row where the run started

  for (let row = window.first; row <= window.last + 1; row++) {
    const local = row - frame.startRow;
    if (local < 0 || local >= frame.rowCount) continue;

    const lane = frame.lanes[local];
    if (lane !== undefined && open.has(lane)) {
      const from = open.get(lane) ?? row;
      strokeVertical(ctx, lane, from, row, metrics, colours, first);
      open.delete(lane);
    }
    for (const parentLane of parentLanesOf(frame, local)) {
      if (!open.has(parentLane)) open.set(parentLane, row);
    }
  }

  // Runs that leave the bottom of the window continue past it.
  for (const [lane, from] of open) {
    strokeVertical(ctx, lane, from, window.last + 1, metrics, colours, first);
  }
}

function strokeVertical(
  ctx: CanvasRenderingContext2D,
  lane: number,
  fromRow: number,
  toRow: number,
  metrics: Metrics,
  colours: string[],
  first: number,
): void {
  if (toRow <= fromRow + 1) return;
  const x = laneX(lane, metrics);
  ctx.strokeStyle = laneColour(lane, colours);
  ctx.beginPath();
  ctx.moveTo(x, rowY(fromRow + 1, first, metrics));
  ctx.lineTo(x, rowY(toRow, first, metrics));
  ctx.stroke();
}

/**
 * Sizes the backing store for the display.
 *
 * Under Wayland fractional scaling the ratio is not an integer, so the backing store is
 * rounded and the transform set once here rather than per frame; leaving it fractional blurs
 * every one-pixel line.
 */
export function resizeCanvas(
  canvas: HTMLCanvasElement,
  cssWidth: number,
  cssHeight: number,
  ratio: number,
): CanvasRenderingContext2D | null {
  const width = Math.max(1, Math.round(cssWidth * ratio));
  const height = Math.max(1, Math.round(cssHeight * ratio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  canvas.style.width = `${cssWidth}px`;
  canvas.style.height = `${cssHeight}px`;

  // willReadFrequently is never set: it pins the canvas to an unaccelerated path for good.
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  return ctx;
}
