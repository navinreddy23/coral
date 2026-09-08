import { describe, expect, it } from 'vitest';

import {
  DEFAULT_METRICS,
  fittedMetrics,
  firstRowFor,
  graphWidthFor,
  isCompressed,
  laneColour,
  laneX,
  maxScroll,
  MAX_SPACER_PX,
  rowsPerScreen,
  listTop,
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
    // And to one the window actually uses: the fallback was an orphan hex that appeared in no
    // token, so a graph drawn without a stylesheet came out a colour from nowhere.
    expect(laneColour(0, [])).toBe('#096cb3');
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
    const lastTop = kernel - rowsPerScreen(viewport, m);
    expect(bottom).toBe(lastTop);

    const middle = firstRowFor((MAX_SPACER_PX - viewport) / 2, viewport, kernel, m);
    expect(middle).toBeCloseTo(lastTop / 2, -2);
  });

  /**
   * The property the assertion above cannot check, because it computes the answer the same way
   * the code does. Scrolled to the bottom, the last row has to be on screen and clear of the
   * bottom edge. This is where the kernel's very first commit used to sit below the fold with
   * nowhere further to scroll.
   */
  it('shows the last row when the scrollbar is at the bottom', () => {
    for (const viewport of [400, 601, 775, 790, 800, 1013]) {
      for (const total of [1_481_528, 200_000, 71_500]) {
        const first = firstRowFor(MAX_SPACER_PX - viewport, viewport, total, m);
        const shown = rowsPerScreen(viewport, m);
        expect(first + shown, `${total} rows in ${viewport}px`).toBeGreaterThanOrEqual(total);
        expect(shown * m.rowHeight, `${shown} rows fit`).toBeLessThanOrEqual(viewport);
      }
    }
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

describe('the width the lanes need', () => {
  const m = DEFAULT_METRICS;

  it('covers the outermost node and leaves it clear of the message', () => {
    // The node at lane 0 is centred at 15 with a radius of 8, so its right edge is at 23.
    expect(graphWidthFor(0, m)).toBeGreaterThan(laneX(0, m) + m.nodeRadius);
    for (const lane of [0, 1, 4, 17]) {
      expect(graphWidthFor(lane, m), `lane ${lane}`).toBeGreaterThanOrEqual(
        laneX(lane, m) + m.nodeRadius,
      );
    }
  });

  it('grows by exactly one lane per lane', () => {
    expect(graphWidthFor(3, m) - graphWidthFor(2, m)).toBe(m.laneWidth);
  });

  it('never returns less than the width of a single lane', () => {
    // A frame can report nothing yet, and a negative width would collapse the canvas.
    expect(graphWidthFor(-1, m)).toBe(graphWidthFor(0, m));
  });
});

describe('row placement', () => {
  /* The list is absolutely positioned inside the scroller, so what reaches the screen is
     `listTop(scrollTop) - scrollTop`. A scroll container reports a fractional scrollTop under
     trackpad and fractional display scaling, and text laid out on a half pixel renders
     blurry — so that difference has to be zero, not merely small. */
  const fractional = [0, 0.5, 12.3333, 411.75, 9999.999, 1e6 + 0.4];

  it('puts the list exactly on the top of the viewport', () => {
    for (const scroll of fractional) {
      expect(listTop(scroll) - scroll, `scroll ${scroll}`).toBe(0);
    }
  });

  it('lands every row on a whole multiple of the row height', () => {
    const { rowHeight } = DEFAULT_METRICS;
    for (const scroll of fractional) {
      for (const index of [0, 1, 17, 500]) {
        const onScreen = listTop(scroll) - scroll + index * rowHeight;
        expect(onScreen % 1, `scroll ${scroll}, row ${index}`).toBe(0);
      }
    }
  });
});

describe('fitting the lanes into the column there is', () => {
  it('draws at the shipped pitch when the lanes fit', () => {
    expect(fittedMetrics(3, 400)).toEqual(DEFAULT_METRICS);
    expect(fittedMetrics(0, 60)).toEqual(DEFAULT_METRICS);
  });

  it('tightens the pitch rather than drawing off the side of the column', () => {
    // Thirty lanes in three hundred pixels: at the shipped pitch that is six hundred, and
    // every node past the edge simply is not on screen.
    const m = fittedMetrics(30, 300);
    expect(m.laneWidth).toBeLessThan(DEFAULT_METRICS.laneWidth);
    expect(laneX(30, m) + m.nodeRadius).toBeLessThanOrEqual(300);
    // The rows stay where they were; only the lanes move.
    expect(m.rowHeight).toBe(DEFAULT_METRICS.rowHeight);
  });

  it('shrinks the node with the lane, so neighbours do not touch', () => {
    const m = fittedMetrics(40, 180);
    expect(m.nodeRadius * 2).toBeLessThanOrEqual(m.laneWidth);
  });

  it('stops tightening rather than drawing lanes nobody can tell apart', () => {
    const m = fittedMetrics(200, 120);
    expect(m.laneWidth).toBeGreaterThanOrEqual(7);
    expect(m.nodeRadius).toBeGreaterThanOrEqual(3);
  });
});

/**
 * A graph just past a screenful, which is most repositories.
 *
 * The rows are pinned under the sticky header and redrawn a whole row at a time rather than
 * scrolled through, so the range has to cover a whole number of rows plus what the last
 * screenful leaves over. It covered only the rows, so on thirty-six commits in a 791px pane
 * the last two could not be reached by the wheel at all.
 */
describe('a graph a little taller than the pane', () => {
  const m = DEFAULT_METRICS;

  it('can scroll far enough to show the last row', () => {
    for (const viewport of [400, 601, 791, 800, 1013]) {
      for (const total of [36, 29, 200, 1000]) {
        const reach = maxScroll(total, m, viewport);
        const first = firstRowFor(reach, viewport, total, m);
        expect(first + rowsPerScreen(viewport, m), `${total} rows in ${viewport}px`)
          .toBeGreaterThanOrEqual(total);
      }
    }
  });

  it('cannot scroll past it', () => {
    const viewport = 791;
    const total = 36;
    const first = firstRowFor(maxScroll(total, m, viewport), viewport, total, m);
    // The topmost row at the bottom of the range still has a full screen of rows under it.
    expect(first).toBeLessThanOrEqual(total - rowsPerScreen(viewport, m));
  });

  it('leaves the height alone before the pane has been measured', () => {
    expect(spacerHeight(36, m)).toBe(36 * m.rowHeight);
  });
});
