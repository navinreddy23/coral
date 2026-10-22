// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));

import { DiffState } from '../src/state/diff.svelte';
import { ViewsState } from '../src/state/views.svelte';

const BINARY = { path: 'logo.png', oldPath: null, binary: true, added: null, removed: null, hunks: [] };

beforeEach(() => {
  invoke.mockReset();
  localStorage.clear();
  invoke.mockResolvedValue(BINARY);
});

describe('the two sides of the file on screen', () => {
  it('are nothing before a file is open', () => {
    expect(new DiffState(new ViewsState()).sides).toBeNull();
  });

  it('are the parent and the commit, for a commit, under the old name before a rename', async () => {
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'abc', 'img/new.png', 'img/old.png');
    expect(diff.sides).toEqual({
      repo: '/repo',
      before: { side: { kind: 'parent', rev: 'abc' }, file: 'img/old.png' },
      after: { side: { kind: 'commit', rev: 'abc' }, file: 'img/new.png' },
    });
  });

  it('are the two commits, when comparing', async () => {
    const diff = new DiffState(new ViewsState());
    await diff.openCompare('/repo', 'old', 'new', 'logo.png');
    expect(diff.sides?.before.side).toEqual({ kind: 'commit', rev: 'old' });
    expect(diff.sides?.after.side).toEqual({ kind: 'commit', rev: 'new' });
  });

  it('are the last commit and the index for a staged change, the index and the disk for the rest', async () => {
    const diff = new DiffState(new ViewsState());
    await diff.openWorking('/repo', true, 'logo.png');
    expect(diff.sides?.before.side).toEqual({ kind: 'commit', rev: 'HEAD' });
    expect(diff.sides?.after.side).toEqual({ kind: 'index' });
    await diff.openWorking('/repo', false, 'logo.png');
    expect(diff.sides?.before.side).toEqual({ kind: 'index' });
    expect(diff.sides?.after.side).toEqual({ kind: 'worktree' });
  });
});
