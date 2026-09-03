import { beforeEach, describe, expect, it, vi } from 'vitest';

import { GRAPH_COLUMN_PX, REFS_COLUMN_PX } from '../src/graph/layout';
import { clampPane, PANE_LIMITS, PanesState } from '../src/state/panes.svelte';

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
