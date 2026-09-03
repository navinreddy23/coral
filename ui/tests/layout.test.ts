import { describe, expect, it } from 'vitest';

import {
  DEFAULT_METRICS,
  firstRowFor,
  isCompressed,
  laneColour,
  laneX,
  MAX_SPACER_PX,
  rowTop,
  rowY,
  spacerHeight,
  visibleRows,
} from '../src/graph/layout';

describe('graph layout', () => {
  it('places lanes at a fixed stride from the origin', () => {
    const m = DEFAULT_METRICS;
    expect(laneX(0, m)).toBe(m.laneOrigin);
    expect(laneX(3, m)).toBe(m.laneOrigin + 3 * m.laneWidth);
  });

  it('centres a row within its band, relative to the first drawn row', () => {
    const m = DEFAULT_METRICS;
    expect(rowY(10, 10, m)).toBe(m.rowHeight / 2);
    expect(rowY(11, 10, m)).toBe(m.rowHeight + m.rowHeight / 2);
  });

  it('overscans by a screen on each side without leaving the graph', () => {
    const m = DEFAULT_METRICS;
    const w = visibleRows(0, 280, 1000, m);

    expect(w.first).toBe(0); // cannot scroll above the first row
    expect(w.last).toBe(20); // ten visible rows plus two screens of overscan
  });

  it('never returns an inverted window when scrolled past the end', () => {
    const w = visibleRows(100000, 280, 50, DEFAULT_METRICS);
    expect(w.last).toBe(49);
    expect(w.first).toBeLessThanOrEqual(w.last);
  });

  it('reports an empty window for an empty graph', () => {
    const w = visibleRows(0, 280, 0, DEFAULT_METRICS);
    expect(w.last).toBeLessThan(w.first);
  });

  it('scrolls the window down with the viewport', () => {
    const m = DEFAULT_METRICS;
    const w = visibleRows(28 * 100, 280, 1000, m);
    expect(w.first).toBe(90);
    expect(w.last).toBe(120);
  });

  it('cycles lane colours so a wide graph never runs out', () => {
    const colours = ['a', 'b', 'c'];
    expect(laneColour(0, colours)).toBe('a');
    expect(laneColour(3, colours)).toBe('a');
    expect(laneColour(4, colours)).toBe('b');
    expect(laneColour(214, colours)).toBe(colours[214 % 3]);
  });

  it('falls back to a colour rather than undefined when none are configured', () => {
    expect(laneColour(0, [])).toBe('#3fa9f5');
  });
});

describe('very tall graphs', () => {
  const m = DEFAULT_METRICS;

  it('scrolls one pixel per pixel while it can', () => {
    expect(isCompressed(1000, m)).toBe(false);
    expect(spacerHeight(1000, m)).toBe(1000 * m.rowHeight);
    expect(firstRowFor(28 * 40, 600, 1000, m)).toBe(40);
  });

  /* 1.48M kernel rows would need 41.5M px, past the point WebKit rescales a layer — which
     shows up as blurry, misaligned content rather than as an error. */
  it('caps the scrollable height for a graph that would exceed the layer limit', () => {
    const kernel = 1_481_528;
    expect(isCompressed(kernel, m)).toBe(true);
    expect(spacerHeight(kernel, m)).toBe(MAX_SPACER_PX);
    expect(spacerHeight(kernel, m)).toBeLessThan(kernel * m.rowHeight);
  });

  it('maps the whole row range onto the capped scrollbar', () => {
    const kernel = 1_481_528;
    const viewport = 800;

    expect(firstRowFor(0, viewport, kernel, m)).toBe(0);

    const bottom = firstRowFor(MAX_SPACER_PX - viewport, viewport, kernel, m);
    const lastTop = kernel - Math.floor(viewport / m.rowHeight);
    expect(bottom).toBe(lastTop);

    const middle = firstRowFor((MAX_SPACER_PX - viewport) / 2, viewport, kernel, m);
    expect(middle).toBeCloseTo(lastTop / 2, -2);
  });

  it('never reports a row outside the graph', () => {
    const kernel = 1_481_528;
    for (const top of [-100, 0, 1, MAX_SPACER_PX, MAX_SPACER_PX * 2]) {
      const row = firstRowFor(top, 800, kernel, m);
      expect(row).toBeGreaterThanOrEqual(0);
      expect(row).toBeLessThan(kernel);
    }
  });

  it('leaves the visible window consistent when compressed', () => {
    const kernel = 1_481_528;
    const w = visibleRows(MAX_SPACER_PX / 2, 800, kernel, m);
    expect(w.first).toBeLessThanOrEqual(w.last);
    expect(w.last).toBeLessThan(kernel);
  });
});

describe('row placement', () => {
  const m = DEFAULT_METRICS;

  /* A scroll container reports a fractional scrollTop under trackpad scrolling, and text laid
     out on a half-pixel renders blurry. Rows are the only thing positioned from it. */
  it('places rows on whole pixels even when the scroll offset is fractional', () => {
    for (const scroll of [0, 0.5, 12.3333, 411.75, 9999.999]) {
      for (const row of [0, 1, 7]) {
        const top = rowTop(scroll, row, 0, m);
        expect(Number.isInteger(top), `scroll ${scroll} row ${row} gave ${top}`).toBe(true);
      }
    }
  });

  it('keeps rows one row-height apart', () => {
    const a = rowTop(100.4, 5, 5, m);
    const b = rowTop(100.4, 6, 5, m);
    expect(b - a).toBe(m.rowHeight);
  });

  it('anchors the first drawn row to the scroll offset', () => {
    expect(rowTop(280, 10, 10, m)).toBe(280);
  });
});
