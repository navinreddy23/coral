import { parentLanesOf, type Frame } from './frame';
import { authorColourIndex } from './initials';
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
  background: string,
  initials: (row: number) => string | null = () => null,
  author: (row: number) => string | null = () => null,
): void {
  ctx.clearRect(0, 0, width, height);
  // An even width centred on an integer coordinate covers whole pixels; 1.5px straddles two
  // and is rendered as two half-lit ones, which reads as a soft line rather than a thin one.
  ctx.lineWidth = 2;
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
        // A right-angled elbow with a rounded corner, as the reference draws it: the edge
        // leaves the node horizontally on the node's own row, turns through a short arc, and
        // runs straight down the parent's lane. A bezier spread over the row instead reads as
        // a lazy diagonal and makes it hard to tell which lane a line ended up in.
        ctx.arcTo(x1, y0, x1, y1, cornerRadius(metrics));
        ctx.lineTo(x1, y1);
      }
      ctx.stroke();
    }
  }

  // Vertical runs for lanes that pass through a row without a node in it.
  drawThroughLanes(ctx, frame, window, metrics, colours, first);

  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.font = `600 ${metrics.nodeRadius}px system-ui, sans-serif`;

  for (let row = window.first; row <= window.last; row++) {
    const local = rowOf(row);
    if (local < 0 || local >= frame.rowCount) continue;

    const lane = frame.lanes[local];
    if (lane === undefined) continue;

    const x = laneX(lane, metrics);
    const y = rowY(row, first, metrics);

    // The node is the author's badge, ringed in its lane's colour: the ring says which line
    // the commit is on, the fill says who wrote it. Both are drawn at full size whether or not
    // the row's metadata has arrived, so a node does not change size under the pointer as a
    // scroll settles. Merges are left unmarked — the two edges leaving the node already say
    // it, and a second ring only crowds the letters.
    const who = author(row);
    const fill =
      who === null
        ? laneColour(lane, colours)
        : colours[authorColourIndex(who, colours.length)] ?? laneColour(lane, colours);

    ctx.beginPath();
    ctx.arc(x, y, metrics.nodeRadius, 0, Math.PI * 2);
    ctx.fillStyle = fill;
    ctx.fill();

    if (who !== null) {
      // Only where the fill can differ from the ring is a ring worth drawing.
      ctx.strokeStyle = laneColour(lane, colours);
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(x, y, metrics.nodeRadius - 1, 0, Math.PI * 2);
      ctx.stroke();
    }

    const label = initials(row);
    if (label !== null) {
      // Every colour in the palette is tuned to 5.5:1 against the page, and contrast is
      // symmetric, so the page colour reads back on top of any of them.
      ctx.fillStyle = background;
      ctx.fillText(label, x, y + 0.5);
    }
  }
}

/**
 * Radius of the turn where an edge changes lane.
 *
 * A third of the lane pitch, matching the reference: large enough to read as a curve rather
 * than a mitre, small enough that the horizontal and vertical runs either side stay obvious.
 */
function cornerRadius(m: Metrics): number {
  return Math.min(m.laneWidth, m.rowHeight) / 3;
}

/**
 * Vertical runs for lanes crossing the window.
 *
 * A lane reserved for a parent stays occupied from the child's row until the parent's own, and
 * on a repository the size of the kernel that span is routinely hundreds of thousands of rows.
 * The child that opened it is almost never on screen, so the run cannot be discovered by
 * scanning the window: it is seeded from the frame's per-row open mask, which says exactly
 * which lanes carry an edge into the first visible row. Scanning alone drew a lane only
 * between two commits that happened to be visible together, which broke the graph into
 * disconnected fragments wherever a branch ran longer than the viewport.
 */
function drawThroughLanes(
  ctx: CanvasRenderingContext2D,
  frame: Frame,
  window: Window,
  metrics: Metrics,
  colours: string[],
  first: number,
): void {
  const half = metrics.rowHeight / 2;
  const topY = rowY(window.first, first, metrics) - half;
  const bottomY = rowY(window.last, first, metrics) + half;

  // lane -> the y it has been running from. Lanes already open above the window start at its
  // top edge, since their opening commit is off screen.
  const open = new Map<number, number>();
  const entering = frame.open[window.first - frame.startRow] ?? 0;
  for (let lane = 0; lane < 32; lane++) {
    if ((entering & (1 << lane)) !== 0) open.set(lane, topY);
  }

  for (let row = window.first; row <= window.last; row++) {
    const local = row - frame.startRow;
    if (local < 0 || local >= frame.rowCount) continue;

    // A run ends at precisely the row drawn in its lane.
    const lane = frame.lanes[local];
    const nodeY = rowY(row, first, metrics);
    if (lane !== undefined) {
      const from = open.get(lane);
      if (from !== undefined) {
        strokeVertical(ctx, lane, from, nodeY, metrics, colours);
        open.delete(lane);
      }
    }
    // The edge pass has already drawn this row's own descent into each parent lane, so the
    // run picks up from the next row's centre. A lane already open is one an earlier child
    // reserved; its run is continuous and must keep its original start.
    for (const parentLane of parentLanesOf(frame, local)) {
      if (!open.has(parentLane)) open.set(parentLane, rowY(row + 1, first, metrics));
    }
  }

  for (const [lane, from] of open) {
    strokeVertical(ctx, lane, from, bottomY, metrics, colours);
  }
}

function strokeVertical(
  ctx: CanvasRenderingContext2D,
  lane: number,
  fromY: number,
  toY: number,
  metrics: Metrics,
  colours: string[],
): void {
  if (toY <= fromY) return;
  const x = laneX(lane, metrics);
  ctx.strokeStyle = laneColour(lane, colours);
  ctx.beginPath();
  ctx.moveTo(x, fromY);
  ctx.lineTo(x, toY);
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
