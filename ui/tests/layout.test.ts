import { describe, expect, it } from 'vitest';

import { DEFAULT_METRICS, laneColour, laneX, rowY, visibleRows } from '../src/graph/layout';

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
