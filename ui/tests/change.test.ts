import { describe, expect, it } from 'vitest';

import { heldFor } from '../src/state/change';

const none = { refs: false, index: false, worktree: false, ops: false, graph: false };

// A change the watcher reports while the window is busy was dropped, and the working copy on
// screen stayed as it was until something else reloaded it.
describe('a change that comes while the window is busy', () => {
  it('is kept, with every later one added to it', () => {
    let held = heldFor(null, { ...none, worktree: true });
    held = heldFor(held, { ...none, index: true });
    expect(held).toEqual({ ...none, worktree: true, index: true });
  });

  it('leaves the refs and the walk to the action that moved them', () => {
    expect(heldFor(null, { ...none, refs: true, graph: true })).toBeNull();
    expect(heldFor(null, { ...none, refs: true, worktree: true })).toEqual({ ...none, worktree: true });
  });
});
