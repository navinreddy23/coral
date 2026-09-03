import { describe, expect, it } from 'vitest';

import { TabsState, type Session, type Tab } from '../src/state/tabs.svelte';

function tab(id: number, path: string, group: number | null = null): Tab {
  return { id, path, group, missing: false };
}

function withSession(session: Session): TabsState {
  const state = new TabsState();
  state.session = session;
  return state;
}

describe('tab bands', () => {
  it('groups consecutive tabs that share a group into one band', () => {
    const state = withSession({
      tabs: [tab(1, '/a', 10), tab(2, '/b', 10), tab(3, '/c')],
      groups: [{ id: 10, name: 'kernel', colour: 'lane1', collapsed: false }],
      active: 1,
    });

    expect(state.bands).toHaveLength(2);
    expect(state.bands[0]?.group?.name).toBe('kernel');
    expect(state.bands[0]?.tabs.map((t) => t.id)).toEqual([1, 2]);
    expect(state.bands[1]?.group).toBeNull();
    expect(state.bands[1]?.tabs.map((t) => t.id)).toEqual([3]);
  });

  it('keeps ungrouped tabs together rather than one band each', () => {
    const state = withSession({
      tabs: [tab(1, '/a'), tab(2, '/b'), tab(3, '/c')],
      groups: [],
      active: 1,
    });

    expect(state.bands).toHaveLength(1);
    expect(state.bands[0]?.tabs).toHaveLength(3);
  });

  it('starts a new band when the group changes', () => {
    const state = withSession({
      tabs: [tab(1, '/a', 10), tab(2, '/b', 11)],
      groups: [
        { id: 10, name: 'one', colour: 'lane1', collapsed: false },
        { id: 11, name: 'two', colour: 'lane2', collapsed: false },
      ],
      active: 1,
    });

    expect(state.bands.map((b) => b.group?.name)).toEqual(['one', 'two']);
  });

  it('reports the active tab, or nothing when it is gone', () => {
    const state = withSession({ tabs: [tab(1, '/a')], groups: [], active: 1 });
    expect(state.active?.path).toBe('/a');

    state.session = { tabs: [tab(1, '/a')], groups: [], active: 99 };
    expect(state.active).toBeNull();
  });

  it('titles a tab by its directory name', () => {
    expect(TabsState.title(tab(1, '/home/me/projects/coral'))).toBe('coral');
    expect(TabsState.title(tab(2, '/home/me/projects/coral/'))).toBe('coral');
    expect(TabsState.title(tab(3, 'coral'))).toBe('coral');
  });

  it('has no bands when nothing is open', () => {
    expect(withSession({ tabs: [], groups: [], active: null }).bands).toEqual([]);
  });
});
