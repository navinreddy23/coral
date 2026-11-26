import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { WorktreesState } from '../src/state/worktrees.svelte';
import type { Worktree } from '../src/ipc/types';

function tree(path: string, over: Partial<Worktree> = {}): Worktree {
  return {
    path,
    head: 'a'.repeat(40),
    branch: null,
    locked: false,
    bare: false,
    main: false,
    ...over,
  };
}

beforeEach(() => invoke.mockReset());

describe('the working trees a repository offers to manage', () => {
  it('leaves out its own, whatever path git reports for it', async () => {
    // Inside a submodule git reports the gitdir under `.git/modules/…` rather than the
    // checkout, so comparing the reported path with the path the repository was opened at
    // found no match: the submodule listed its own working tree as a linked one and offered
    // to remove the very thing being looked at. git's ordering says which is which.
    invoke.mockResolvedValue([
      tree('/repo/.git/modules/vendor/sub', { main: true, branch: 'main' }),
    ]);
    const trees = new WorktreesState();
    await trees.load('/repo/vendor/sub');
    expect(trees.all).toHaveLength(1);
    expect(trees.others).toEqual([]);
  });

  it('lists the ones made beside it', async () => {
    invoke.mockResolvedValue([
      tree('/repo', { main: true, branch: 'main' }),
      tree('/tmp/wt', { branch: 'topic' }),
    ]);
    const trees = new WorktreesState();
    await trees.load('/repo');
    expect(trees.others.map((w) => w.path)).toEqual(['/tmp/wt']);
  });

  it('lists the main one, and not itself, in a tab open on one made beside it', async () => {
    // Listing the linked trees left this one in the list and the main one out, so its own tab
    // offered to remove the folder it was showing, which git does from inside it.
    invoke.mockResolvedValue([
      tree('/repo', { main: true, branch: 'main' }),
      tree('/tmp/wt', { branch: 'topic' }),
      tree('/tmp/other', { branch: 'fix' }),
    ]);
    const trees = new WorktreesState();
    await trees.load('/tmp/wt');
    expect(trees.others.map((w) => w.path)).toEqual(['/repo', '/tmp/other']);
  });

  it('leaves out a bare repository, which has no files of its own', async () => {
    invoke.mockResolvedValue([tree('/repo.git', { main: true, bare: true })]);
    const trees = new WorktreesState();
    await trees.load('/repo.git');
    expect(trees.others).toEqual([]);
  });

  it('empties itself when the answer fails, rather than keeping the last one', async () => {
    invoke.mockImplementation((name: string) =>
      name === 'repo_worktrees'
        ? Promise.reject(new Error('not a repository'))
        : Promise.resolve(null),
    );
    const trees = new WorktreesState();
    await trees.load('/gone');
    expect(trees.all).toEqual([]);
    expect(trees.error).toContain('not a repository');
  });
});
