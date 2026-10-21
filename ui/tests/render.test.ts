import { describe, expect, it } from 'vitest';

import type { Frame } from '../src/graph/frame';
import { DEFAULT_METRICS, laneX, rowY, type Window } from '../src/graph/layout';
import { drawLanes } from '../src/graph/render';

interface Segment {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** A cubic edge from one point to another, with the two control points that shape it. */
interface Curve {
  x0: number;
  y0: number;
  c1x: number;
  c1y: number;
  c2x: number;
  c2y: number;
  x1: number;
  y1: number;
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
 * Only the geometry matters here, so fills are accepted and dropped rather than mocked
 * faithfully; a bezier is always an edge between two adjacent rows.
 */
function recorder(): {
  ctx: CanvasRenderingContext2D;
  segments: Segment[];
  curves: Curve[];
  discs: Disc[];
  rings: Disc[];
  labels: Label[];
} {
  const segments: Segment[] = [];
  const curves: Curve[] = [];
  const labels: Label[] = [];
  const discs: Disc[] = [];
  const rings: Disc[] = [];
  let pendingArc: Disc | null = null;
  let at = { x: 0, y: 0 };
  let pending: Segment | null = null;
  const ctx = {
    lineWidth: 0,
    lineCap: '',
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
      pending = { x0: at.x, y0: at.y, x1: x, y1: y };
      at = { x, y };
    },
    bezierCurveTo(c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number) {
      curves.push({ x0: at.x, y0: at.y, c1x, c1y, c2x, c2y, x1: x, y1: y });
      at = { x, y };
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
      if (pending) segments.push(pending);
      pending = null;
    },
  };
  return {
    ctx: ctx as unknown as CanvasRenderingContext2D,
    segments,
    curves,
    discs,
    rings,
    labels,
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
    // Filled from the node palette, ringed in the lane's own colour. Row 0 is a merge, which
    // is drawn differently and has a test of its own.
    const plain = discs.slice(1);
    expect(plain.every((d) => d.fill === '#fill')).toBe(true);
    expect(plain.every((d) => d.stroke === '#a')).toBe(true);
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

    drawLanes(ctx, frame, { first: 1, last: 2 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#lane'],
      fills,
      initials: () => 'LT',
      author: (row) => (row === 1 ? 'a@example.com' : 'b@example.com'),
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
    expect(discs.slice(1).every((d) => d.radius >= DEFAULT_METRICS.nodeRadius - 1)).toBe(true);
  });

  it('draws a merge as a smaller dot in its lane colour, with no letters on it', () => {
    // A run of merges is structure, not content: the kernel's first-parent line is thousands
    // of them, each the same author's initials in the same disc. The two edges leaving the dot
    // already say what it is.
    const frame = longRunFrame(20, 2);
    const { ctx, discs, labels } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 1 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#lane'],
      fills: ['#fill'],
      initials: () => 'LT',
    });

    const [merge, plain] = discs;
    expect(merge?.fill, 'filled in its lane colour').toBe('#lane');
    expect(merge?.radius ?? 0).toBeLessThan((plain?.radius ?? 0) / 2 + 1);
    expect(merge?.radius ?? 0).toBeGreaterThan(2);
    expect(labels.map((l) => l.y), 'letters on the plain node only').toEqual([
      rowY(1, 0, DEFAULT_METRICS) + 0.5,
    ]);
  });
});

describe('an edge that changes lane', () => {
  it('leaves its node heading down and arrives in the parent lane heading down', () => {
    // Not a diagonal: a curve that is vertical at both ends says which lane it left and which
    // it reached, and a diagonal across the row says neither.
    const frame = longRunFrame(20, 3);
    const { ctx, curves } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a', '#b', '#c', '#d'],
      fills: ['#fill'],
    });

    // Row 0 opens lane 3 for its second parent; that is the only lane change in the window.
    expect(curves).toHaveLength(1);
    const curve = curves[0];
    const from = laneX(0, DEFAULT_METRICS);
    const to = laneX(3, DEFAULT_METRICS);
    expect([curve?.x0, curve?.y0]).toEqual([from, rowY(0, 0, DEFAULT_METRICS)]);
    expect([curve?.x1, curve?.y1]).toEqual([to, rowY(1, 0, DEFAULT_METRICS)]);
    expect(curve?.c1x, 'leaves vertically').toBe(from);
    expect(curve?.c2x, 'arrives vertically').toBe(to);
  });

  it('draws no curve where the edge stays in its lane', () => {
    const frame = longRunFrame(20, 3);
    const { ctx, curves } = recorder();
    // A window past row 0 has only the straight first-parent run in it.
    drawLanes(ctx, frame, { first: 5, last: 8 }, DEFAULT_METRICS, 200, 200, {
      colours: ['#a', '#b', '#c', '#d'],
      fills: ['#fill'],
    });
    expect(curves).toHaveLength(0);
  });
});

describe('a graph drawn tight', () => {
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
