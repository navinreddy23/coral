import { beforeEach, describe, expect, it, vi } from 'vitest';

import { GRAPH_COLUMN_PX, REFS_COLUMN_PX } from '../src/graph/layout';
import {
  clampPane,
  fitColumns,
  MESSAGE_FLOOR_PX,
  PANE_LIMITS,
  PanesState,
} from '../src/state/panes.svelte';

/** Storage the tests can inspect, so the widths can be checked without a browser. */
function stubStorage(): Map<string, string> {
  const store = new Map<string, string>();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => store.set(k, v),
  });
  return store;
}

describe('pane widths', () => {
  let store = new Map<string, string>();
  beforeEach(() => {
    store = stubStorage();
  });

  it('starts at the shipped layout', () => {
    const panes = new PanesState();
    expect(panes.widths.refs).toBe(REFS_COLUMN_PX);
    expect(panes.widths.graph).toBe(GRAPH_COLUMN_PX);
  });

  it('never lets a drag close a pane past its handle', () => {
    const panes = new PanesState();
    for (const key of ['sidebar', 'details', 'refs', 'graph'] as const) {
      panes.resize(key, -10_000);
      expect(panes.widths[key], key).toBe(PANE_LIMITS[key].min);
      panes.resize(key, 10_000);
      expect(panes.widths[key], key).toBe(PANE_LIMITS[key].max);
    }
  });

  it('remembers a drag across a restart', () => {
    new PanesState().resize('sidebar', 300);
    expect(new PanesState().widths.sidebar).toBe(300);
  });

  it('clamps a width stored under limits that have since changed', () => {
    store.set('coral.panes', JSON.stringify({ sidebar: 4, details: 99_999 }));
    const panes = new PanesState();
    expect(panes.widths.sidebar).toBe(PANE_LIMITS.sidebar.min);
    expect(panes.widths.details).toBe(PANE_LIMITS.details.max);
    // A key the stored object never had falls back rather than becoming NaN.
    expect(panes.widths.refs).toBe(REFS_COLUMN_PX);
  });

  it('opens with the shipped layout when the stored entry is nonsense', () => {
    // A hand-edited or truncated entry must not stop the window opening.
    for (const junk of ['', '{', 'null', '[]', '{"sidebar":"wide"}']) {
      store.set('coral.panes', junk);
      expect(new PanesState().widths.sidebar, junk).toBe(240);
    }
  });

  it('rounds to whole pixels, so a row never lands on a half', () => {
    expect(clampPane('refs', 190.6)).toBe(191);
  });

  it('puts everything back', () => {
    const panes = new PanesState();
    panes.resize('graph', 400);
    panes.reset();
    expect(panes.widths.graph).toBe(GRAPH_COLUMN_PX);
  });
});

describe('the graph column following the lanes', () => {
  beforeEach(() => {
    stubStorage();
  });

  it('grows to the lanes it is asked to fit', () => {
    const panes = new PanesState();
    panes.refit();
    expect(panes.widths.graph).toBe(PANE_LIMITS.graph.min);
    panes.fitGraph(200);
    expect(panes.widths.graph).toBe(200);
  });

  it('never shrinks while a repository is open', () => {
    // A column that also shrank would shift every commit message sideways each time a merge
    // cluster scrolled off the screen, which is worse than a little unused width.
    const panes = new PanesState();
    panes.refit();
    panes.fitGraph(200);
    panes.fitGraph(90);
    expect(panes.widths.graph).toBe(200);
  });

  it('starts again for the next repository', () => {
    const panes = new PanesState();
    panes.fitGraph(300);
    panes.refit();
    expect(panes.widths.graph).toBe(PANE_LIMITS.graph.min);
  });

  it('keeps a hand-set width when the next repository starts again', () => {
    const panes = new PanesState();
    panes.resize('graph', 220);
    panes.refit();
    expect(panes.widths.graph).toBe(220);
  });

  it('remembers that it was pinned, across a restart', () => {
    new PanesState().resize('graph', 220);
    const next = new PanesState();
    next.refit();
    expect(next.widths.graph).toBe(220);
  });

  it('still widens a hand-set column that cannot hold the lanes', () => {
    // A drag settles how narrow the column may be, not how wide. Stopping the fit for good
    // left every node drawn off the side of a column nothing would widen again, in the merge
    // stretches of the kernel where the graph is at its widest.
    const panes = new PanesState();
    panes.resize('graph', 220);
    panes.fitGraph(400);
    expect(panes.widths.graph).toBe(400);
    // And never narrower than the width that was chosen.
    panes.fitGraph(100);
    expect(panes.widths.graph).toBe(400);
  });

  it('follows again after a reset', () => {
    const panes = new PanesState();
    panes.resize('graph', 220);
    panes.reset();
    panes.fitGraph(300);
    expect(panes.widths.graph).toBe(300);
  });

  it('keeps the fitted width inside the handle limits', () => {
    const panes = new PanesState();
    panes.fitGraph(10_000);
    expect(panes.widths.graph).toBe(PANE_LIMITS.graph.max);
  });
});

describe('columns in a pane too narrow for them', () => {
  const widths = { sidebar: 240, details: 340, refs: 240, graph: 170 };

  it('leaves them alone where there is room', () => {
    expect(fitColumns(widths, 1200)).toEqual({ refs: 240, graph: 170 });
  });

  /**
   * The window's own minimum is 900 wide; with both side panels at their shipped widths the
   * commit list gets 320 of it, and 240 + 170 is more than that. The message column was given
   * nothing, so the list showed no messages, no dates and no object ids at all.
   */
  it('makes room for the message column at the window minimum', () => {
    const fitted = fitColumns(widths, 320);
    expect(fitted.refs + fitted.graph).toBeLessThan(320);
    expect(320 - fitted.refs - fitted.graph).toBeGreaterThan(150);
  });

  it('never shrinks either column past the width that keeps its handle reachable', () => {
    const fitted = fitColumns(widths, 100);
    expect(fitted.refs).toBe(PANE_LIMITS.refs.min);
    expect(fitted.graph).toBe(PANE_LIMITS.graph.min);
  });

  it('starts scaling exactly where the floor stops fitting', () => {
    const both = widths.refs + widths.graph;
    expect(fitColumns(widths, both + MESSAGE_FLOOR_PX)).toEqual({ refs: 240, graph: 170 });
    const tighter = fitColumns(widths, both + MESSAGE_FLOOR_PX - 40);
    expect(tighter.refs + tighter.graph).toBeLessThan(both);
  });

  it('answers before the pane has been measured', () => {
    expect(fitColumns(widths, 0)).toEqual({ refs: 240, graph: 170 });
  });
});
