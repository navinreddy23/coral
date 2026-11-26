import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

const { WorktreeState } = await import('../src/state/worktree.svelte');

const empty = { entries: [], branch: 'main', upstream: null, ahead: 0, behind: 0 };

/** A status read that answers only when told to. */
function held() {
  let answer: (value: unknown) => void = () => {};
  invoke.mockImplementationOnce(() => new Promise((resolve) => (answer = resolve)));
  return () => answer(empty);
}

// While a read is in flight the window drops what the watcher says changed, so a busy left on
// is a window that stops noticing edits made outside it until the tab is opened again.
describe('the working copy being read', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('is not busy once a repository being read is left', async () => {
    const worktree = new WorktreeState();
    const answer = held();
    const reading = worktree.load('/kernel');
    expect(worktree.busy).toBe(true);

    worktree.clear();
    answer();
    await reading;

    expect(worktree.busy).toBe(false);
  });

  it('stays busy until the read of the repository now open answers', async () => {
    const worktree = new WorktreeState();
    const first = held();
    const leaving = worktree.load('/kernel');
    const second = held();
    const arriving = worktree.load('/alice');

    first();
    await leaving;
    expect(worktree.busy).toBe(true);

    second();
    await arriving;
    expect(worktree.busy).toBe(false);
  });
});
