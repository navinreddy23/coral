import { describe, expect, it } from 'vitest';

import { bandWidth, columnWidth, laneToken } from '../src/graph/column';
import type { Frame } from '../src/graph/frame';
import { DEFAULT_METRICS, graphWidthFor } from '../src/graph/layout';

/** A frame whose row `n` sits in lane `lanes[n]`, which is all these three read. */
function frameOf(lanes: number[]): Frame {
  const rows = lanes.length;
  return {
    startRow: 0,
    rowCount: rows,
    totalRows: rows,
    flags: 0,
    hashLen: 20,
    lanes: Uint16Array.from(lanes),
    rowFlags: new Uint8Array(rows),
    times: new Float64Array(rows),
    parentStart: new Uint32Array(rows + 1),
    parentLanes: new Uint16Array(0),
    oids: new Uint8Array(rows * 20),
    open: new Uint32Array(rows),
  };
}

const WIDE = 1400;

describe('how wide the lane column wants to be', () => {
  it('asks for what the widest row on screen needs', () => {
    const frame = frameOf([0, 3, 1, 0]);
    expect(columnWidth(frame, [0, 1, 2, 3], WIDE, DEFAULT_METRICS)).toBe(
      graphWidthFor(3, DEFAULT_METRICS),
    );
  });

  it('asks for nothing at all before there are rows to draw', () => {
    expect(columnWidth(frameOf([0]), [], WIDE, DEFAULT_METRICS)).toBe(0);
  });

  it('will not take the commit message with it in a merge region', () => {
    // The kernel draws thirty lanes at once. A graph beside no message is not worth the trade,
    // so past a share of the pane the lanes are drawn tighter instead of the column growing.
    const frame = frameOf(Array.from({ length: 40 }, (_, i) => i));
    const rows = frame.lanes.length;
    const pane = 900;
    const got = columnWidth(frame, [...Array(rows).keys()], pane, DEFAULT_METRICS);
    expect(got).toBeLessThan(graphWidthFor(39, DEFAULT_METRICS));
    expect(got).toBe(Math.round(pane * 0.35));
  });
});

describe('the band between a row’s lane and its message', () => {
  it('fills the corridor a linear row leaves in a column sized for a merge', () => {
    // The column is as wide as the widest row on screen and never narrows again, so a row in
    // lane 0 has most of it empty. That emptiness is what the lane colour fills.
    const frame = frameOf([0, 0, 6, 0]);
    const px = graphWidthFor(6, DEFAULT_METRICS);
    const wide = bandWidth(frame, 0, px, DEFAULT_METRICS, [0, 1, 2, 3]);
    const tight = bandWidth(frame, 2, px, DEFAULT_METRICS, [0, 1, 2, 3]);
    expect(wide).toBeGreaterThan(tight);
    // On the widest row there is no corridor: what is left is the margin the column reserves
    // past the last node so a ring is not drawn against the text.
    expect(tight, 'the widest row keeps its lanes').toBeLessThanOrEqual(
      DEFAULT_METRICS.nodeRadius + 10,
    );
  });

  it('is nothing for a row the frame does not reach', () => {
    // A row past the loaded window has no lane to measure from, and a band drawn for it would
    // be a colour with no commit under it.
    expect(bandWidth(frameOf([0, 0]), 99, 200, DEFAULT_METRICS, [0, 1])).toBe(0);
    expect(bandWidth(null, 0, 200, DEFAULT_METRICS, [0])).toBe(0);
  });

  it('never runs past the column it sits in', () => {
    const frame = frameOf([0, 1, 2]);
    for (const row of [0, 1, 2]) {
      const got = bandWidth(frame, row, 120, DEFAULT_METRICS, [0, 1, 2]);
      expect(got).toBeGreaterThanOrEqual(0);
      expect(got).toBeLessThanOrEqual(120);
    }
  });
});

describe('which of the eight colours a row came out', () => {
  it('numbers them from one, the way the tokens are named', () => {
    expect(laneToken(frameOf([0, 1, 7]), 0)).toBe(1);
    expect(laneToken(frameOf([0, 1, 7]), 2)).toBe(8);
  });

  it('repeats the eight, exactly as the canvas repeats them', () => {
    expect(laneToken(frameOf([8]), 0)).toBe(1);
    expect(laneToken(frameOf([17]), 0)).toBe(2);
  });

  it('answers the first colour for a row there is no frame for', () => {
    expect(laneToken(null, 0)).toBe(1);
    expect(laneToken(frameOf([0]), 50)).toBe(1);
  });
});
