// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { SelectionState } from '../src/state/selection.svelte';
import { DiffState } from '../src/state/diff.svelte';
import { ViewsState } from '../src/state/views.svelte';

const DETAIL = {
  commit: {
    oid: 'a'.repeat(40),
    summary: 'a commit',
    body: '',
    author: { name: 'A', email: 'a@b', time: 0, offset: 0 },
    committer: { name: 'A', email: 'a@b', time: 0, offset: 0 },
    parents: [],
  },
  files: [{ path: 'one.txt', oldPath: null, change: 'modified' }],
};

const DIFFER = [
  { path: 'one.txt', oldPath: null, change: 'modified' },
  { path: 'two.txt', oldPath: null, change: 'added' },
];

beforeEach(() => {
  invoke.mockReset();
  localStorage.clear();
  invoke.mockImplementation((name: string) => {
    const table: Record<string, unknown> = {
      commit_detail: DETAIL,
      compare_commits: DIFFER,
      compare_file_diff: null,
      file_diff: null,
    };
    return Promise.resolve(table[name] ?? null);
  });
});

function called(name: string) {
  return invoke.mock.calls.filter((c) => c[0] === name);
}

describe('comparing two commits', () => {
  it('puts the older of the two first, whichever was picked first', async () => {
    // Rows count back through history, so the larger row number is the older commit.
    const oldest = { row: 40, oid: 'o'.repeat(40) };
    const newest = { row: 4, oid: 'n'.repeat(40) };

    const forward = new SelectionState();
    await forward.select('/repo', newest.row, newest.oid);
    await forward.compare('/repo', oldest.row, oldest.oid);
    expect(forward.pair).toEqual({ from: oldest, to: newest });

    const backward = new SelectionState();
    await backward.select('/repo', oldest.row, oldest.oid);
    await backward.compare('/repo', newest.row, newest.oid);
    expect(backward.pair).toEqual({ from: oldest, to: newest });

    expect(called('compare_commits')[0]?.[1]).toMatchObject({
      from: oldest.oid,
      to: newest.oid,
    });
  });

  it("lists what differs, in place of the one commit's own files", async () => {
    const selection = new SelectionState();
    await selection.select('/repo', 4, 'n'.repeat(40));
    expect(selection.detail?.files).toHaveLength(1);

    await selection.compare('/repo', 40, 'o'.repeat(40));
    expect(selection.compared).toHaveLength(2);
    // No single commit's details, because there is no single commit.
    expect(selection.detail).toBeNull();
  });

  it('marks both ends of the pair, so the list shows what is being compared', async () => {
    const selection = new SelectionState();
    await selection.select('/repo', 4, 'n'.repeat(40));
    await selection.compare('/repo', 40, 'o'.repeat(40));

    expect(selection.marks(4)).toBe(true);
    expect(selection.marks(40)).toBe(true);
    expect(selection.marks(12)).toBe(false);
  });

  it('selects outright when there is nothing to compare with', async () => {
    const selection = new SelectionState();
    await selection.compare('/repo', 4, 'n'.repeat(40));

    expect(selection.pair).toBeNull();
    expect(selection.row).toBe(4);
    expect(called('compare_commits')).toHaveLength(0);
  });

  it('will not compare a commit with itself', async () => {
    const selection = new SelectionState();
    await selection.select('/repo', 4, 'n'.repeat(40));
    await selection.compare('/repo', 4, 'n'.repeat(40));

    expect(selection.pair).toBeNull();
    expect(called('compare_commits')).toHaveLength(0);
  });

  it('reads a file across the pair rather than out of one commit', async () => {
    const diff = new DiffState(new ViewsState());
    await diff.openCompare('/repo', 'o'.repeat(40), 'n'.repeat(40), 'one.txt');

    expect(called('file_diff')).toHaveLength(0);
    expect(called('compare_file_diff')[0]?.[1]).toMatchObject({
      from: 'o'.repeat(40),
      to: 'n'.repeat(40),
      file: 'one.txt',
    });
    expect(diff.error).toBe('That file is the same in both commits.');
  });

  it('picking one commit again ends the comparison', async () => {
    const selection = new SelectionState();
    await selection.select('/repo', 4, 'n'.repeat(40));
    await selection.compare('/repo', 40, 'o'.repeat(40));
    await selection.select('/repo', 9, 'x'.repeat(40));

    expect(selection.pair).toBeNull();
    expect(selection.compared).toHaveLength(0);
    expect(selection.marks(40)).toBe(false);
  });
});
