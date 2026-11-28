import { describe, expect, it } from 'vitest';

import type { Frame } from '../src/graph/frame';
import { DEFAULT_METRICS, laneX, rowY, type Window } from '../src/graph/layout';
import { drawLanes } from '../src/graph/render';

interface Segment {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  alpha: number;
}

interface Corner {
  cx: number;
  cy: number;
  x: number;
  y: number;
  radius: number;
}

interface Disc {
  x: number;
  y: number;
  radius: number;
  fill: string;
  stroke: string;
}

interface Label {
  text: string;
  x: number;
  y: number;
  fill: string;
}

/**
 * Records the straight strokes a render produces.
 *
 * Only the geometry matters here, so curves and fills are accepted and dropped rather than
 * mocked faithfully; a bezier is always an edge between two adjacent rows.
 */
function recorder(): {
  ctx: CanvasRenderingContext2D;
  segments: Segment[];
  corners: Corner[];
  discs: Disc[];
  rings: Disc[];
  labels: Label[];
  images: { x: number; y: number; size: number; clipped: boolean }[];
} {
  const segments: Segment[] = [];
  const corners: Corner[] = [];
  const labels: Label[] = [];
  const discs: Disc[] = [];
  const rings: Disc[] = [];
  const images: { x: number; y: number; size: number; clipped: boolean }[] = [];
  let clipped = false;
  let pendingArc: Disc | null = null;
  let at = { x: 0, y: 0 };
  let pending: Segment | null = null;
  const ctx = {
    lineWidth: 0,
    lineCap: '',
    globalAlpha: 1,
    strokeStyle: '',
    fillStyle: '',
    clearRect() {},
    beginPath() {
      pending = null;
    },
    moveTo(x: number, y: number) {
      at = { x, y };
    },
    lineTo(x: number, y: number) {
      pending = { x0: at.x, y0: at.y, x1: x, y1: y, alpha: 1 };
      at = { x, y };
    },
    bezierCurveTo() {
      pending = null;
    },
    /**
     * The rounded corner an edge turns through when it changes lane. The arc ends on the
     * segment towards the second point, `radius` away from the corner, which is where the
     * straight run that follows it starts.
     */
    arcTo(cx: number, cy: number, x: number, y: number, radius: number) {
      corners.push({ cx, cy, x, y, radius });
      const dx = x - cx;
      const dy = y - cy;
      const len = Math.hypot(dx, dy) || 1;
      at = { x: cx + (dx / len) * radius, y: cy + (dy / len) * radius };
      pending = null;
    },
    arc(x: number, y: number, radius: number) {
      pendingArc = { x, y, radius, fill: '', stroke: '' };
      pending = null;
    },
    /** A node is filled and then stroked, so it is recorded once both colours are known. */
    fill() {
      if (pendingArc) pendingArc.fill = String(ctx.fillStyle);
    },
    save() {},
    restore() {
      clipped = false;
    },
    clip() {
      clipped = true;
    },
    drawImage(_image: unknown, x: number, y: number, w: number) {
      images.push({ x: x + w / 2, y: y + w / 2, size: w, clipped });
    },
    fillText(text: string, x: number, y: number) {
      labels.push({ text, x, y, fill: String(ctx.fillStyle) });
    },
    stroke() {
      if (pendingArc) {
        pendingArc.stroke = String(ctx.strokeStyle);
        // A circle that was never filled is a ring around something else, not a node.
        (pendingArc.fill === '' ? rings : discs).push(pendingArc);
        pendingArc = null;
        return;
      }
      if (pending) segments.push({ ...pending, alpha: ctx.globalAlpha });
      pending = null;
    },
  };
  return {
    ctx: ctx as unknown as CanvasRenderingContext2D,
    segments,
    corners,
    discs,
    rings,
    labels,
    images,
  };
}

/**
 * A chain of `rows` commits in lane 0 whose first row also opens `lane` for a parent at the
 * very bottom — the shape of a long-lived branch, and of the kernel's first-parent line.
 */
function longRunFrame(rows: number, lane: number): Frame {
  const lanes = new Uint16Array(rows);
  const parentStart = new Uint32Array(rows + 1);
  const parentLanes: number[] = [];
  const open = new Uint32Array(rows);

  for (let row = 0; row < rows; row++) {
    lanes[row] = 0;
    if (row === 0) {
      parentLanes.push(0, lane);
    } else if (row < rows - 1) {
      parentLanes.push(0);
    }
    parentStart[row + 1] = parentLanes.length;
    // Lane 0 is live from row 1 on; the reserved lane is live from row 1 to the end.
    open[row] = row === 0 ? 0 : 1 | (1 << lane);
  }

  return {
    startRow: 0,
    rowCount: rows,
    totalRows: rows,
    flags: 0,
    hashLen: 20,
    lanes,
    rowFlags: new Uint8Array(rows),
    times: new Float64Array(rows),
    parentStart,
    parentLanes: Uint16Array.from(parentLanes),
    oids: new Uint8Array(rows * 20),
    open,
    joins: new Uint32Array(rows),
  };
}

interface Shape {
  lane: number;
  /** The lane each parent's line runs down from this row. */
  parents: number[];
  /** Lanes turning into this row's node. */
  joins?: number[];
}

/**
 * A frame for a hand-written layout, with the open mask a renderer would derive by following
 * each line down to the first row drawn in its lane or turning it in.
 */
function frameOf(shape: Shape[]): Frame {
  const rows = shape.length;
  const joinMask = (s: Shape) => (s.joins ?? []).reduce((m, l) => m | (1 << l), 0);
  const open = new Uint32Array(rows);
  shape.forEach((row, i) => {
    for (const lane of row.parents) {
      for (let r = i + 1; r < rows; r++) {
        open[r] = (open[r] ?? 0) | (1 << lane);
        const below = shape[r];
        if (below === undefined || below.lane === lane || (joinMask(below) & (1 << lane)) !== 0) break;
      }
    }
  });
  const parentStart = new Uint32Array(rows + 1);
  const parentLanes: number[] = [];
  shape.forEach((row, i) => {
    parentLanes.push(...row.parents);
    parentStart[i + 1] = parentLanes.length;
  });
  return {
    startRow: 0,
    rowCount: rows,
    totalRows: rows,
    flags: 0,
    hashLen: 20,
    lanes: Uint16Array.from(shape.map((r) => r.lane)),
    rowFlags: new Uint8Array(rows),
    times: new Float64Array(rows),
    parentStart,
    parentLanes: Uint16Array.from(parentLanes),
    oids: new Uint8Array(rows * 20),
    open,
    joins: Uint32Array.from(shape.map(joinMask)),
  };
}

function verticalsIn(lane: number, segments: Segment[]): Segment[] {
  const x = laneX(lane, DEFAULT_METRICS);
  return segments.filter((s) => s.x0 === x && s.x1 === x && s.y1 > s.y0);
}

describe('drawLanes', () => {
  it('draws a lane whose commit and parent are both outside the window', () => {
    const frame = longRunFrame(400, 3);
    const window: Window = { first: 150, last: 170 };
    const { ctx, segments } = recorder();

    drawLanes(ctx, frame, window, DEFAULT_METRICS, 120, 600, {
      colours: ['#a', '#b', '#c', '#d'],
      fills: ['#fill'],
    });

    const run = verticalsIn(3, segments);
    expect(run.length).toBeGreaterThan(0);
    const height = DEFAULT_METRICS.rowHeight;
    const top = Math.min(...run.map((s) => s.y0));
    const bottom = Math.max(...run.map((s) => s.y1));
    // Edge to edge: the run enters at the top of the window and leaves at the bottom.
    expect(top).toBeLessThanOrEqual(0);
    expect(bottom).toBeGreaterThanOrEqual((window.last - window.first) * height);
  });

  it('spans the window for every lane the open mask reports', () => {
    const rows = 200;
    const frame = longRunFrame(rows, 5);
    // Three more long-lived branches running alongside, as a busy repository has.
    for (let row = 1; row < rows; row++) {
      frame.open[row] = (frame.open[row] ?? 0) | (1 << 2) | (1 << 6) | (1 << 9);
    }

    const window: Window = { first: 80, last: 100 };
    const { ctx, segments } = recorder();
    drawLanes(ctx, frame, window, DEFAULT_METRICS, 200, 600, {
      colours: Array(12).fill('#000'),
      fills: ['#fill'],
    });

    for (const lane of [2, 5, 6, 9]) {
      const covered = verticalsIn(lane, segments).reduce((n, s) => n + (s.y1 - s.y0), 0);
      expect(covered, `lane ${lane} is not drawn across the window`).toBeGreaterThanOrEqual(
        (window.last - window.first) * DEFAULT_METRICS.rowHeight,
      );
    }
  });

  it('stops a run at the commit that consumes it', () => {
    const rows = 100;
    const frame = longRunFrame(rows, 4);
    // Lane 4 is consumed at row 50: from there down nothing enters it.
    frame.lanes[50] = 4;
    for (let row = 51; row < rows; row++) {
      frame.open[row] = (frame.open[row] ?? 0) & ~(1 << 4);
    }

    const window: Window = { first: 40, last: 60 };
    const { ctx, segments } = recorder();
    drawLanes(ctx, frame, window, DEFAULT_METRICS, 200, 600, {
      colours: Array(8).fill('#000'),
      fills: ['#fill'],
    });

    const bottom = Math.max(...verticalsIn(4, segments).map((s) => s.y1));
    // The node's own row centre, and not a pixel below it.
    expect(bottom).toBe((50 - window.first) * DEFAULT_METRICS.rowHeight + DEFAULT_METRICS.rowHeight / 2);
  });
});


describe('drawing a node', () => {
  it('is a disc from the node palette, ringed in the lane colour', () => {
    const frame = longRunFrame(20, 2);
    const window: Window = { first: 0, last: 3 };
    const { ctx, discs } = recorder();

    drawLanes(ctx, frame, window, DEFAULT_METRICS, 200, 200, {
      colours: ['#a', '#b', '#c'],
      fills: ['#fill'],
    });

    // One per visible row, on its own lane.
    expect(discs.length).toBe(4);
    expect(discs.every((d) => d.x === laneX(0, DEFAULT_METRICS))).toBe(true);
    // Filled from the node palette, ringed in the lane's own colour.
    expect(discs.every((d) => d.fill === '#fill')).toBe(true);
    expect(discs.every((d) => d.stroke === '#a')).toBe(true);
  });

  it('carries the author initials inside the ring', () => {
    const frame = longRunFrame(20, 2);
    const { ctx, labels } = recorder();

    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a'],
      fills: ['#fill'],
      initials: (row) => (row === 1 ? 'LT' : null),
    });

    expect(labels).toHaveLength(1);
    expect(labels[0]?.text).toBe('LT');
    expect(labels[0]?.x).toBe(laneX(0, DEFAULT_METRICS));
  });

  it('colours the disc by author, and the letters on it white', () => {
    const frame = longRunFrame(20, 2);
    const fills = ['#f0', '#f1', '#f2', '#f3'];
    const { ctx, discs, labels } = recorder();

    drawLanes(ctx, frame, { first: 0, last: 1 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#lane'],
      fills,
      initials: () => 'LT',
      author: (row) => (row === 0 ? 'a@example.com' : 'b@example.com'),
    });

    // Two authors, two fills; the same author would give the same one on every row.
    expect(discs).toHaveLength(2);
    expect(fills).toContain(discs[0]?.fill);
    expect(discs[0]?.fill).not.toBe(discs[1]?.fill);
    expect(labels.every((l) => l.text === 'LT')).toBe(true);
    expect(labels.every((l) => l.fill === '#ffffff')).toBe(true);
  });

  it('leaves a node blank while its metadata is still loading', () => {
    const frame = longRunFrame(20, 2);
    const { ctx, labels, discs } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a'],
      fills: ['#fill'],
    });
    // The disc is drawn at full size either way, so a node does not change under the pointer
    // as a scroll settles.
    expect(labels).toHaveLength(0);
    expect(discs).toHaveLength(4);
  });

  it('stays inside the radius the layout reserved for it', () => {
    // The ring is stroked, and a stroke straddles its path. Drawing it at the full radius put
    // half its width outside what `graphWidthFor` allows for, against the message beside it.
    const frame = longRunFrame(20, 2);
    const { ctx, discs } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a'],
      fills: ['#fill'],
    });

    expect(discs.length).toBe(4);
    expect(discs.every((d) => d.radius < DEFAULT_METRICS.nodeRadius)).toBe(true);
    expect(discs.every((d) => d.radius >= DEFAULT_METRICS.nodeRadius - 1)).toBe(true);
  });
});

describe('an edge that changes lane', () => {
  it('turns through a rounded corner rather than a lazy diagonal', () => {
    const frame = longRunFrame(20, 3);
    const { ctx, corners } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a', '#b', '#c', '#d'],
      fills: ['#fill'],
    });

    // Row 0 opens lane 3 for its second parent; that is the only lane change in the window.
    expect(corners).toHaveLength(1);
    const corner = corners[0];
    // Horizontal out of the node, then straight down the parent's lane: the corner sits at the
    // parent lane's x on the node's own row, and the arc ends heading downwards.
    expect(corner?.cx).toBe(laneX(3, DEFAULT_METRICS));
    expect(corner?.cy).toBe(rowY(0, 0, DEFAULT_METRICS));
    expect(corner?.x).toBe(laneX(3, DEFAULT_METRICS));
    expect(corner?.y).toBeGreaterThan(corner?.cy ?? 0);
    expect(corner?.radius).toBeGreaterThan(0);
  });

  it('draws no corner where the edge stays in its lane', () => {
    const frame = longRunFrame(20, 3);
    const { ctx, corners } = recorder();
    // A window past row 0 has only the straight first-parent run in it.
    drawLanes(ctx, frame, { first: 5, last: 8 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a', '#b', '#c', '#d'],
      fills: ['#fill'],
    });
    expect(corners).toHaveLength(0);
  });
});

describe('a branch', () => {
  // main, a branch tip beside it, main again, then the commit the branch left from.
  const forked: Shape[] = [
    { lane: 0, parents: [0] },
    { lane: 1, parents: [1] },
    { lane: 0, parents: [0] },
    { lane: 0, parents: [0], joins: [1] },
    { lane: 0, parents: [] },
  ];
  const half = DEFAULT_METRICS.rowHeight / 2;

  it('runs down its own lane and turns into the commit it left from on that commit’s row', () => {
    const { ctx, corners, segments } = recorder();
    drawLanes(ctx, frameOf(forked), { first: 0, last: 4 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#main', '#branch'],
      fills: ['#fill'],
    });

    expect(corners).toHaveLength(1);
    const corner = corners[0];
    // The corner is in the branch's lane on the parent's row, and the arc heads for its node.
    expect(corner?.cx).toBe(laneX(1, DEFAULT_METRICS));
    expect(corner?.cy).toBe(rowY(3, 0, DEFAULT_METRICS));
    expect(corner?.x).toBe(laneX(0, DEFAULT_METRICS));
    expect(corner?.y).toBe(rowY(3, 0, DEFAULT_METRICS));
    // Straight down the branch's lane from its tip to the top of that row, and no further.
    const run = verticalsIn(1, segments);
    expect(Math.min(...run.map((s) => s.y0))).toBe(rowY(1, 0, DEFAULT_METRICS));
    expect(Math.max(...run.map((s) => s.y1))).toBe(rowY(3, 0, DEFAULT_METRICS) - half);
  });

  it('turns in from the top of the row when the commit it left from is the next one down', () => {
    const { ctx, segments } = recorder();
    const adjacent: Shape[] = [
      { lane: 0, parents: [0] },
      { lane: 1, parents: [1] },
      { lane: 0, parents: [], joins: [1] },
    ];
    drawLanes(ctx, frameOf(adjacent), { first: 0, last: 2 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#main', '#branch'],
      fills: ['#fill'],
    });
    // Nothing of the branch's lane reaches past the top of the row it turns into: a straight
    // line run on to the node's centre would poke out under the corner.
    const lowest = Math.max(...verticalsIn(1, segments).map((s) => s.y1));
    expect(lowest).toBeLessThanOrEqual(rowY(2, 0, DEFAULT_METRICS) - half);
  });

  it('is drawn in its own colour as it turns in', () => {
    const strokes: string[] = [];
    const { ctx } = recorder();
    const arcTo = ctx.arcTo.bind(ctx);
    ctx.arcTo = (...args: Parameters<CanvasRenderingContext2D['arcTo']>) => {
      strokes.push(String(ctx.strokeStyle));
      arcTo(...args);
    };
    drawLanes(ctx, frameOf(forked), { first: 0, last: 4 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#main', '#branch'],
      fills: ['#fill'],
    });
    expect(strokes).toEqual(['#branch']);
  });

  it('turns several in from the right, the nearest drawn last', () => {
    const fan: Shape[] = [
      { lane: 0, parents: [0] },
      { lane: 1, parents: [1] },
      { lane: 2, parents: [2] },
      { lane: 0, parents: [], joins: [1, 2] },
    ];
    const { ctx, corners } = recorder();
    drawLanes(ctx, frameOf(fan), { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a', '#b', '#c'],
      fills: ['#fill'],
    });
    expect(corners.map((c) => c.cx)).toEqual([laneX(2, DEFAULT_METRICS), laneX(1, DEFAULT_METRICS)]);
    expect(corners.every((c) => c.cx > laneX(0, DEFAULT_METRICS))).toBe(true);
  });
});

describe('a line given back for running too long', () => {
  it('stops at the row the mask closes it on, and does not run into the next commit in its lane', () => {
    const frame = longRunFrame(40, 3);
    // Lane 3 is given back at row 20, and a new tip takes it at row 30.
    for (let row = 20; row < 40; row++) frame.open[row] = (frame.open[row] ?? 0) & ~(1 << 3);
    frame.lanes[30] = 3;
    const { ctx, segments } = recorder();
    drawLanes(ctx, frame, { first: 10, last: 35 }, DEFAULT_METRICS, 200, 2000, {
      colours: Array(4).fill('#000'),
      fills: ['#fill'],
    });

    const run = verticalsIn(3, segments);
    const half = DEFAULT_METRICS.rowHeight / 2;
    expect(Math.max(...run.map((s) => s.y1))).toBe(rowY(20, 10, DEFAULT_METRICS) - half);
  });

  it('fades out over the row above where it stops, so it reads as going on out of view', () => {
    const frame = longRunFrame(40, 3);
    for (let row = 20; row < 40; row++) frame.open[row] = (frame.open[row] ?? 0) & ~(1 << 3);
    frame.lanes[30] = 3;
    const { ctx, segments } = recorder();
    drawLanes(ctx, frame, { first: 10, last: 35 }, DEFAULT_METRICS, 200, 2000, {
      colours: Array(4).fill('#000'),
      fills: ['#fill'],
    });

    // A later commit in the lane draws its own line; only the run that stops is read here.
    const stop = rowY(20, 10, DEFAULT_METRICS) - DEFAULT_METRICS.rowHeight / 2;
    const run = verticalsIn(3, segments).filter((s) => s.y1 <= stop);
    const fadeFrom = stop - DEFAULT_METRICS.rowHeight;
    const solid = run.filter((s) => s.y1 <= fadeFrom);
    const fading = run.filter((s) => s.y0 >= fadeFrom).sort((a, b) => a.y0 - b.y0);
    expect(solid.length).toBeGreaterThan(0);
    expect(solid.every((s) => s.alpha === 1)).toBe(true);
    expect(fading.length).toBeGreaterThan(2);
    for (let i = 1; i < fading.length; i++) {
      expect(fading[i]?.alpha ?? 1).toBeLessThan(fading[i - 1]?.alpha ?? 0);
    }
    expect(fading.at(-1)?.alpha ?? 1).toBeLessThan(0.25);
    expect(ctx.globalAlpha).toBe(1);
  });

  it('does not fade a line that reaches its commit', () => {
    const frame = longRunFrame(40, 3);
    const { ctx, segments } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 39 }, DEFAULT_METRICS, 200, 2000, {
      colours: Array(4).fill('#000'),
      fills: ['#fill'],
    });
    expect(verticalsIn(3, segments).every((s) => s.alpha === 1)).toBe(true);
  });
});

describe('a graph drawn tight', () => {
  it('puts the author’s picture inside the ring, in place of the initials', () => {
    const frame = longRunFrame(20, 2);
    const { ctx, discs, labels, images } = recorder();
    const face = {} as CanvasImageSource;

    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#lane'],
      fills: ['#fill'],
      initials: () => 'LT',
      picture: (row) => (row === 1 ? face : null),
    });

    // Row 1 has a picture, clipped to its disc and centred on it; the rest keep their letters.
    expect(images).toHaveLength(1);
    expect(images[0]?.clipped).toBe(true);
    expect(images[0]?.x).toBe(laneX(0, DEFAULT_METRICS));
    expect(images[0]?.size).toBeLessThan(DEFAULT_METRICS.nodeRadius * 2);
    expect(labels).toHaveLength(3);
    // Every node is still ringed in its lane, picture or not.
    expect(discs).toHaveLength(4);
    expect(discs.every((d) => d.stroke === '#lane')).toBe(true);
  });

  it('leaves the picture off a node too small to show it', () => {
    const frame = longRunFrame(20, 2);
    const tight = { ...DEFAULT_METRICS, laneWidth: 7, nodeRadius: 3, laneOrigin: 5 };
    const { ctx, images } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 3 }, tight, 200, 200, {
      colours: ['#a'],
      fills: ['#fill'],
      picture: () => ({}) as CanvasImageSource,
    });
    expect(images).toHaveLength(0);
  });

  it('leaves the letters off a node too small to hold them', () => {
    const frame = longRunFrame(20, 2);
    const tight = { ...DEFAULT_METRICS, laneWidth: 7, nodeRadius: 3, laneOrigin: 5 };
    const { ctx, discs, labels } = recorder();

    drawLanes(ctx, frame, { first: 0, last: 3 }, tight, 200, 200, {
      colours: ['#a'],
      fills: ['#fill'],
      initials: () => 'LT',
    });

    // The nodes are still drawn; it is only the letters in them that would be a smudge.
    expect(discs).toHaveLength(4);
    expect(labels).toHaveLength(0);
  });
});

describe('the commit that is checked out', () => {
  it('is ringed, clear of its own node, in the colour it was given', () => {
    const frame = longRunFrame(20, 2);
    const { ctx, discs, rings } = recorder();

    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#lane'],
      fills: ['#fill'],
      headRow: 2,
      headRing: '#brand',
    });

    expect(rings).toHaveLength(1);
    expect(rings[0]?.stroke).toBe('#brand');
    expect(rings[0]?.y).toBe(rowY(2, 0, DEFAULT_METRICS));
    // Outside the node, so the lane colour ringing it is still readable underneath.
    expect(rings[0]?.radius).toBeGreaterThan(DEFAULT_METRICS.nodeRadius);
    // And the four nodes are drawn as they always were.
    expect(discs).toHaveLength(4);
  });

  it('is ringed nowhere when nothing says which row it is on', () => {
    const frame = longRunFrame(20, 2);
    const { ctx, rings } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#lane'],
      fills: ['#fill'],
    });
    expect(rings).toHaveLength(0);
  });

  it('keeps the ring inside its own row and off the lane beside it, however tight the graph', () => {
    // A kernel merge region is thirty lanes in a column sized for eight, and HEAD is usually
    // the first row on screen. A ring at a fixed offset ran into the next lane in the first
    // case and was cut off by the top of the canvas in the second.
    const frame = longRunFrame(20, 2);
    for (const metrics of [
      DEFAULT_METRICS,
      { ...DEFAULT_METRICS, rowHeight: 24 },
      { ...DEFAULT_METRICS, rowHeight: 32 },
      { ...DEFAULT_METRICS, laneWidth: 7, nodeRadius: 3, laneOrigin: 5 },
    ]) {
      const { ctx, rings } = recorder();
      drawLanes(ctx, frame, { first: 0, last: 3 }, metrics, 200, 200, {
        colours: ['#lane'],
        fills: ['#fill'],
        headRow: 2,
        headRing: '#brand',
      });

      const edge = (rings[0]?.radius ?? 0) + 1;
      expect(edge, 'outside its own node').toBeGreaterThan(metrics.nodeRadius);
      // Half a row is all there is above a node before the row boundary, and the canvas is cut
      // off there when the row is the first one drawn.
      expect(edge, 'inside its own row').toBeLessThanOrEqual(metrics.rowHeight / 2);
      // The next lane along is a two-pixel line at the lane pitch.
      expect(edge, 'clear of the next lane').toBeLessThan(metrics.laneWidth);
    }
  });
});
