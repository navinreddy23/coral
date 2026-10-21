import { NO_LANE, parentLanesOf, type Frame } from './frame';
import { authorColourIndex } from './initials';
import { laneColour, laneX, rowY, type Metrics, type Window } from './layout';

/** Thickness of a node's ring. Two whole pixels, for the reason `lineWidth` is set to two. */
const NODE_STROKE = 2;

/**
 * The letters inside a node.
 *
 * White in both themes, which is why the fills they sit on are their own palette rather than
 * the lane one: see `--node-1` in tokens.css.
 */
const NODE_LABEL = '#ffffff';

/** Smallest node that can hold two letters legibly. */
const LABELLED_FROM = 7;

/** Everything about a lane column that is not its geometry. */
export interface LaneStyle {
  /** Ring colour per lane, cycled. */
  colours: string[];
  /** Node fill per author, cycled, and the fill of a row whose author has not arrived. */
  fills: string[];
  /** Author initials for a row, or null while its metadata is still loading. */
  initials?: (row: number) => string | null;
  /** The author's identity, which fixes the node's fill. Null while it is loading. */
  author?: (row: number) => string | null;
  /** The row HEAD is on, or null when it is off screen or on nothing this frame knows. */
  headRow?: number | null;
  /** What to ring that row with. The brand colour, read from the stylesheet by the caller. */
  headRing?: string;
}

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
  width: number,
  height: number,
  style: LaneStyle,
): void {
  const { colours, fills } = style;
  const initials = style.initials ?? (() => null);
  const author = style.author ?? (() => null);
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
      // A parent the assigner had no lane left for; there is nowhere to draw the edge to.
      if (parentLane === NO_LANE) continue;
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

  // A disc in the author's colour, ringed in its lane's, with the author's initials on it in
  // white. The ring says which line the commit is on, the fill and the letters say who wrote
  // it. Both are drawn at full size whether or not the row's metadata has arrived, so a node
  // does not change size under the pointer as a scroll settles. Merges are left unmarked: the
  // two edges leaving the node already say it.
  ctx.lineWidth = NODE_STROKE;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.font = `600 ${labelSize(metrics)}px system-ui, sans-serif`;

  for (let row = window.first; row <= window.last; row++) {
    const local = rowOf(row);
    if (local < 0 || local >= frame.rowCount) continue;

    const lane = frame.lanes[local];
    if (lane === undefined) continue;

    const x = laneX(lane, metrics);
    const y = rowY(row, first, metrics);
    // Stroked inside the radius, so the outer edge is where the metrics say it is whatever the
    // ring is drawn at.
    const radius = metrics.nodeRadius - NODE_STROKE / 2;
    const who = author(row);

    ctx.beginPath();
    ctx.arc(x, y, radius, 0, Math.PI * 2);
    ctx.fillStyle =
      who === null
        ? laneColour(lane, fills)
        : fills[authorColourIndex(who, fills.length)] ?? laneColour(lane, fills);
    ctx.fill();
    ctx.strokeStyle = laneColour(lane, colours);
    ctx.stroke();

    // The commit that is checked out, ringed clear of its own node so the lane colour under it
    // stays readable. The gap is left unpainted rather than filled: the canvas is transparent
    // over the row, so what shows through is the row's own background, selected or not.
    if (row === style.headRow) {
      ctx.strokeStyle = style.headRing ?? laneColour(lane, colours);
      ctx.beginPath();
      ctx.arc(x, y, headRadius(metrics), 0, Math.PI * 2);
      ctx.stroke();
    }

    // Only where there is room for them to be read. In a merge region the lanes are drawn
    // tight and the node is a few pixels across; two letters in it are a smudge, and a smudge
    // in every node is worse than a plain dot.
    const label = metrics.nodeRadius >= LABELLED_FROM ? initials(row) : null;
    if (label !== null) {
      ctx.fillStyle = NODE_LABEL;
      ctx.fillText(label, x, y + 0.5);
    }
  }
}

/**
 * Where the ring on the HEAD node sits.
 *
 * Three pixels outside the node where there is room, and less where there is not. It has to
 * stay inside its own row: HEAD is usually near the top of the graph, and a ring drawn at a
 * fixed offset had its top cut off by the edge of the canvas whenever HEAD was the first row
 * on screen. The lane pitch bounds it too — at the pitch a kernel merge region is drawn at,
 * a ring three pixels out crosses the line of the next lane along.
 *
 * The stroke straddles the path, so the mark itself reaches a pixel past this on each side.
 */
function headRadius(m: Metrics): number {
  const clear = Math.min(m.laneWidth - 2, m.rowHeight / 2 - 2);
  return Math.max(m.nodeRadius + 1, Math.min(m.nodeRadius + 3, clear));
}

/**
 * Type size for the two letters inside a node.
 *
 * Sized off the ring rather than fixed, and small enough that two capitals clear the stroke:
 * at the ring's own radius they touched it on both sides and the node read as a smudge.
 */
function labelSize(m: Metrics): number {
  return Math.round(m.nodeRadius * 0.85);
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
      if (parentLane === NO_LANE) continue;
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
