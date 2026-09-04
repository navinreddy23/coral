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
  labels: Label[];
} {
  const segments: Segment[] = [];
  const corners: Corner[] = [];
  const labels: Label[] = [];
  const discs: Disc[] = [];
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
    fillText(text: string, x: number, y: number) {
      labels.push({ text, x, y, fill: String(ctx.fillStyle) });
    },
    stroke() {
      if (pendingArc) {
        pendingArc.stroke = String(ctx.strokeStyle);
        discs.push(pendingArc);
        pendingArc = null;
        return;
      }
      if (pending) segments.push(pending);
      pending = null;
    },
  };
  return { ctx: ctx as unknown as CanvasRenderingContext2D, segments, corners, discs, labels };
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

    drawLanes(ctx, frame, window, DEFAULT_METRICS, ['#a', '#b', '#c', '#d'], 120, 600, ['#fill']);

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
    for (let row = 1; row < rows; row++) frame.open[row] |= (1 << 2) | (1 << 6) | (1 << 9);

    const window: Window = { first: 80, last: 100 };
    const { ctx, segments } = recorder();
    drawLanes(ctx, frame, window, DEFAULT_METRICS, Array(12).fill('#000'), 200, 600, ['#fill']);

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
    for (let row = 51; row < rows; row++) frame.open[row] &= ~(1 << 4);

    const window: Window = { first: 40, last: 60 };
    const { ctx, segments } = recorder();
    drawLanes(ctx, frame, window, DEFAULT_METRICS, Array(8).fill('#000'), 200, 600, ['#fill']);

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

    drawLanes(ctx, frame, window, DEFAULT_METRICS, ['#a', '#b', '#c'], 200, 200, ['#fill']);

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

    drawLanes(
      ctx,
      frame,
      { first: 0, last: 3 },
      DEFAULT_METRICS,
      ['#a'],
      200,
      200,
      ['#fill'],
      (row) => (row === 1 ? 'LT' : null),
    );

    expect(labels).toHaveLength(1);
    expect(labels[0]?.text).toBe('LT');
    expect(labels[0]?.x).toBe(laneX(0, DEFAULT_METRICS));
  });

  it('colours the disc by author, and the letters on it white', () => {
    const frame = longRunFrame(20, 2);
    const fills = ['#f0', '#f1', '#f2', '#f3'];
    const { ctx, discs, labels } = recorder();

    drawLanes(
      ctx,
      frame,
      { first: 0, last: 1 },
      DEFAULT_METRICS,
      ['#lane'],
      200,
      200,
      fills,
      () => 'LT',
      (row) => (row === 0 ? 'a@example.com' : 'b@example.com'),
    );

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
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, ['#a'], 200, 200, ['#fill']);
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
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, ['#a'], 200, 200, ['#fill']);

    expect(discs.length).toBe(4);
    expect(discs.every((d) => d.radius < DEFAULT_METRICS.nodeRadius)).toBe(true);
    expect(discs.every((d) => d.radius >= DEFAULT_METRICS.nodeRadius - 1)).toBe(true);
  });
});

describe('an edge that changes lane', () => {
  it('turns through a rounded corner rather than a lazy diagonal', () => {
    const frame = longRunFrame(20, 3);
    const { ctx, corners } = recorder();
    drawLanes(ctx, frame, { first: 0, last: 3 }, DEFAULT_METRICS, ['#a', '#b', '#c', '#d'], 200, 200, ['#fill']);

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
    drawLanes(ctx, frame, { first: 5, last: 8 }, DEFAULT_METRICS, ['#a', '#b', '#c', '#d'], 200, 200, ['#fill']);
    expect(corners).toHaveLength(0);
  });
});
