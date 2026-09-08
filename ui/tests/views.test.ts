// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from 'vitest';

import { ViewsState } from '../src/state/views.svelte';

describe('remembered view choices', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('survives a restart, which is the whole point of them', () => {
    const first = new ViewsState();
    first.set('diff', 'split');
    first.set('changes', 'path');

    // A second instance is what the next launch builds.
    const second = new ViewsState();
    expect(second.current.diff).toBe('split');
    expect(second.current.changes).toBe('path');
  });

  it('starts on the shipped choices when nothing has been stored', () => {
    const views = new ViewsState();
    expect(views.current.diff).toBe('inline');
    expect(views.current.commitFiles).toBe('path');
    expect(views.current.sidebar).toBe(true);
    // Remote and tag lists run to hundreds on a real repository.
    expect(views.current.collapsed).toEqual({ remote: true, tags: true });
  });

  it('remembers which sidebar sections were closed on purpose', () => {
    const views = new ViewsState();
    views.setCollapsed('tags', false);
    views.setCollapsed('submodules', true);

    const next = new ViewsState();
    expect(next.current.collapsed['tags']).toBe(false);
    expect(next.current.collapsed['submodules']).toBe(true);
  });

  it('ignores a stored value that names a mode no longer offered', () => {
    // Written by an older release. Taken at face value it leaves a panel rendering nothing.
    localStorage.setItem(
      'coral.views',
      JSON.stringify({ diff: 'three-way', changes: 'tree', sidebar: 'yes' }),
    );
    const views = new ViewsState();
    expect(views.current.diff).toBe('inline');
    expect(views.current.changes, 'a value that is still valid is kept').toBe('tree');
    expect(views.current.sidebar).toBe(true);
  });

  it('clamps a stored terminal size into the range the splitter allows', () => {
    localStorage.setItem('coral.views', JSON.stringify({ terminalSize: 9000 }));
    expect(new ViewsState().current.terminalSize).toBe(900);

    localStorage.setItem('coral.views', JSON.stringify({ terminalSize: -3 }));
    expect(new ViewsState().current.terminalSize).toBe(120);
  });

  it('opens on the defaults when the stored entry is not JSON at all', () => {
    localStorage.setItem('coral.views', 'not json');
    expect(new ViewsState().current.diff).toBe('inline');
  });

  it('resets everything, for when something has been left unusable', () => {
    const views = new ViewsState();
    views.set('sidebar', false);
    views.set('details', false);
    views.reset();

    expect(views.current.sidebar).toBe(true);
    expect(new ViewsState().current.details).toBe(true);
  });
});

describe('which shell the terminal runs', () => {
  it('leaves both to the machine until somebody chooses', () => {
    // An empty shell means "whatever this machine would use" and a null login means "whatever
    // this platform does", which is the only sensible default: macOS gets its PATH from the
    // login files and cannot skip them, and a Linux desktop has already read them.
    const views = new ViewsState();
    expect(views.current.terminalShell).toBe('');
    expect(views.current.terminalLogin).toBeNull();
  });

  it('remembers a choice, including turning the login files off', () => {
    const first = new ViewsState();
    first.set('terminalShell', '/usr/bin/fish');
    first.set('terminalLogin', false);

    const second = new ViewsState();
    expect(second.current.terminalShell).toBe('/usr/bin/fish');
    expect(second.current.terminalLogin, 'false is a choice, not an absence').toBe(false);
  });

  it('ignores a stored login setting that is neither a decision nor an absence', () => {
    localStorage.setItem('coral.views', JSON.stringify({ terminalLogin: 'yes please' }));
    expect(new ViewsState().current.terminalLogin).toBeNull();
  });
});
