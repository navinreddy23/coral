import { describe, expect, it } from 'vitest';

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { DEFAULT_METRICS, ROW_HEIGHTS, fittedMetrics, metricsFor } from '../src/graph/layout';
import { TOKENS_CSS, blocks, declarations } from './stylesheet';

/**
 * How tall a row is, which two different things have to agree on.
 *
 * The rows are laid out by the stylesheet and the lanes beside them are drawn on a canvas.
 * Neither can read the other, so the three numbers are written twice — and this is what keeps
 * the second copy honest. Without it the lanes drift off the grid the text sits on, which
 * looks like a rendering fault and is a data one.
 */

/** `--row-h` as declared for each density in the token file. */
function fromCss(): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [selector, body] of blocks(TOKENS_CSS)) {
    const found = declarations(body).find(([name]) => name === 'row-h');
    if (!found) continue;
    const density = /data-density='([a-z]+)'/.exec(selector)?.[1] ?? 'default';
    out[density] = Number.parseInt(found[1], 10);
  }
  return out;
}

describe('the three densities', () => {
  it('are the same three in the stylesheet and in the canvas metrics', () => {
    expect(fromCss()).toEqual(ROW_HEIGHTS);
  });

  it('run from tightest to loosest', () => {
    expect(ROW_HEIGHTS.compact).toBeLessThan(ROW_HEIGHTS.default);
    expect(ROW_HEIGHTS.default).toBeLessThan(ROW_HEIGHTS.comfortable);
  });

  it('ship dense, because this window is read on a repository with a million commits', () => {
    expect(ROW_HEIGHTS.default).toBe(DEFAULT_METRICS.rowHeight);
  });

  it('change the row height and nothing else about the drawing', () => {
    // A lane's pitch and a node's radius are about how much graph fits across the column, not
    // about how much air a row has.
    for (const density of ['compact', 'comfortable'] as const) {
      const m = metricsFor(density);
      expect(m.rowHeight).toBe(ROW_HEIGHTS[density]);
      expect(m.laneWidth).toBe(DEFAULT_METRICS.laneWidth);
      expect(m.nodeRadius).toBe(DEFAULT_METRICS.nodeRadius);
      expect(m.laneOrigin).toBe(DEFAULT_METRICS.laneOrigin);
    }
  });

  it('survive being squeezed into a narrow lane column', () => {
    // A kernel merge region needs thirty lanes in a column sized for eight, so the pitch is
    // tightened — and the row height has to come through that unchanged.
    for (const density of ['compact', 'default', 'comfortable'] as const) {
      const fitted = fittedMetrics(30, 300, metricsFor(density));
      expect(fitted.rowHeight, density).toBe(ROW_HEIGHTS[density]);
      expect(fitted.laneWidth).toBeLessThan(DEFAULT_METRICS.laneWidth);
    }
  });

  it('leave the shipped metrics alone when nothing is passed', () => {
    expect(fittedMetrics(3, 400)).toEqual(DEFAULT_METRICS);
  });
});

describe('the canvas beside the rows', () => {
  /**
   * The lanes are drawn at whatever row height the caller laid its rows out at.
   *
   * `fittedMetrics` defaults to the shipped metrics, so a canvas that called it with the width
   * alone drew a 28px grid beside 24px rows: every node a pixel further from its own commit
   * than the last, and by the bottom of the screen a node against the wrong message. Nothing
   * in a unit test can see that, because a canvas has no 2d context outside a browser, so the
   * wiring itself is what is checked.
   */
  it('is fitted from the metrics it is given, not the ones it ships with', () => {
    const source = readFileSync(
      resolve(import.meta.dirname, '../src/graph/GraphCanvas.svelte'),
      'utf8',
    );
    expect(source).toContain('fittedMetrics(maxLane, width, base)');
  });
});
