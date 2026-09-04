// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { DiffState } from '../src/state/diff.svelte';
import { ViewsState } from '../src/state/views.svelte';

const EMPTY_DIFF = {
  path: 'a.c',
  oldPath: null,
  change: 'modified',
  binary: false,
  added: 0,
  removed: 0,
  tooLarge: false,
  hunks: [],
};

const BLAME = {
  chunks: [{ oid: 'a'.repeat(40), finalLine: 1, origLine: 1, lines: 2 }],
  commits: {
    ['a'.repeat(40)]: {
      oid: 'a'.repeat(40),
      author: { name: 'Ada', email: 'ada@example.com', time: 1_700_000_000, offset: 0 },
      summary: 'first',
      filename: 'a.c',
    },
  },
};

beforeEach(() => {
  invoke.mockReset();
  localStorage.clear();
});

/** Answers whichever command was asked, so the order of the calls is not baked in. */
function answering(over: Record<string, unknown> = {}) {
  invoke.mockImplementation((name: string) => {
    const table: Record<string, unknown> = {
      file_diff: EMPTY_DIFF,
      worktree_diff: EMPTY_DIFF,
      file_blame: BLAME,
      file_text: 'one\ntwo\n',
      file_history: [],
      ...over,
    };
    return Promise.resolve(table[name] ?? null);
  });
}

function called(name: string) {
  return invoke.mock.calls.filter((c) => c[0] === name);
}

describe('the file panel', () => {
  it('asks for nothing beyond the diff until a view that needs it is chosen', async () => {
    answering();
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'HEAD', 'a.c');

    // Blame and history are each a walk of the file's whole history.
    expect(called('file_blame')).toHaveLength(0);
    expect(called('file_history')).toHaveLength(0);

    diff.setView('blame');
    await vi.waitFor(() => expect(diff.blame).not.toBeNull());
    expect(diff.text).toBe('one\ntwo\n');
  });

  it('blames the commit being shown, not the branch it is on', async () => {
    answering();
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'deadbeef', 'a.c');
    diff.setView('blame');
    await vi.waitFor(() => expect(diff.blame).not.toBeNull());

    expect(called('file_blame')[0]?.[1]).toMatchObject({ rev: 'deadbeef', file: 'a.c' });
  });

  it('blames HEAD for the working tree, which has no commit of its own', async () => {
    answering();
    const diff = new DiffState(new ViewsState());
    await diff.openWorking('/repo', false, 'a.c');
    diff.setView('blame');
    await vi.waitFor(() => expect(diff.blame).not.toBeNull());

    expect(called('file_blame')[0]?.[1]).toMatchObject({ rev: 'HEAD' });
  });

  it('reads the file again when whitespace stops counting', async () => {
    answering();
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'HEAD', 'a.c');
    expect(called('file_diff')[0]?.[1]).toMatchObject({ ignoreWhitespace: false });

    diff.setIgnoreWhitespace(true);
    await vi.waitFor(() => expect(called('file_diff')).toHaveLength(2));
    expect(called('file_diff')[1]?.[1]).toMatchObject({ ignoreWhitespace: true });

    // Setting it to what it already is costs nothing.
    diff.setIgnoreWhitespace(true);
    expect(called('file_diff')).toHaveLength(2);
  });

  it('walks the history from the commit being looked at, not from HEAD', async () => {
    // A file added after the checked-out revision has no history at all when walked from
    // there, which read as "nothing has touched this file" about a file two commits changed.
    answering();
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'deadbeef', 'a.c');
    diff.setView('history');
    await vi.waitFor(() => expect(called('file_history')).toHaveLength(1));
    expect(called('file_history')[0]?.[1]).toMatchObject({ rev: 'deadbeef', file: 'a.c' });

    await diff.openWorking('/repo', false, 'a.c');
    await vi.waitFor(() => expect(called('file_history')).toHaveLength(2));
    expect(called('file_history')[1]?.[1]).toMatchObject({ rev: 'HEAD' });
  });

  it('shows one commit from the history, and finds its way back', async () => {
    answering();
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'HEAD', 'a.c');

    await diff.showCommit('c0ffee');
    expect(diff.atCommit).toBe('c0ffee');
    expect(called('file_diff').at(-1)?.[1]).toMatchObject({ rev: 'c0ffee' });

    await diff.showOpened();
    expect(diff.atCommit).toBeNull();
    expect(called('file_diff').at(-1)?.[1]).toMatchObject({ rev: 'HEAD' });
  });

  it('asks for another page of history and says when there may be more', async () => {
    const page = Array.from({ length: 50 }, (_, i) => ({
      oid: `${i}`,
      summary: 's',
      body: '',
      author: { name: 'A', email: 'a@b', time: 0, offset: 0 },
      committer: { name: 'A', email: 'a@b', time: 0, offset: 0 },
      parents: [],
    }));
    answering({ file_history: page });
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'HEAD', 'a.c');
    diff.setView('history');
    await vi.waitFor(() => expect(diff.history).toHaveLength(50));
    expect(diff.moreHistory).toBe(true);

    await diff.deeper();
    expect(called('file_history').at(-1)?.[1]).toMatchObject({ limit: 100 });
  });

  it('forgets the blame and the history when another file is opened', async () => {
    answering();
    const diff = new DiffState(new ViewsState());
    await diff.open('/repo', 'HEAD', 'a.c');
    diff.setView('blame');
    await vi.waitFor(() => expect(diff.blame).not.toBeNull());

    // Blanked at the start of the load, not left showing the last file's authors.
    const second = diff.open('/repo', 'HEAD', 'b.c');
    expect(diff.blame).toBeNull();
    expect(diff.text).toBeNull();
    await second;
  });
});
