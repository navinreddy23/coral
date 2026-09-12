// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Searching the history, driven through the shell.
 *
 * The engine's matching is `coral-core`'s business. What is checked here is what the window
 * does with the answer: the field counts the matches, so the one it says you are on has to be
 * the one you are looking at.
 */
const frameBytes = readFileSync(resolve(process.cwd(), 'tests/fixtures/frame.bin'));

/** The object ids the fixture frame actually holds, so the metadata stub is about those rows. */
const frameOids = (() => {
  const copy = frameBytes.buffer.slice(
    frameBytes.byteOffset,
    frameBytes.byteOffset + frameBytes.byteLength,
  );
  const frame = decodeFrame(copy);
  return Array.from({ length: frame.rowCount }, (_, r) => oidOf(frame, r));
})();

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import App from '../../src/app/App.svelte';
import type { PlacedRef } from '../../src/ipc/commands';
import { decodeFrame, oidOf } from '../../src/graph/frame';

const frame = decodeFrame(
  frameBytes.buffer.slice(frameBytes.byteOffset, frameBytes.byteOffset + frameBytes.byteLength),
);
const REPO = '/repo';
const SESSION = {
  tabs: [{ id: 1, path: REPO, submodule: null, group: null, missing: false }],
  active: 1,
  groups: [],
};

function answers(): Record<string, unknown> {
  return {
    initial_repo: REPO,
    open_repo: {
      path: REPO,
      gitDir: `${REPO}/.git`,
      gitVersion: '2.43.0',
      head: { kind: 'branch', name: 'master' },
      state: 'clean',
      commitGraph: true,
    },
    binary_self_test: Uint8Array.from({ length: 4096 }, (_, i) => i % 251).buffer,
    graph_frame: frameBytes.buffer.slice(
      frameBytes.byteOffset,
      frameBytes.byteOffset + frameBytes.byteLength,
    ),
    // Keyed by object id in the window, so a stub that invented ids would be answering about
    // commits that are not on those rows.
    row_metadata: frameOids.map((oid, i) => ({
      oid,
      author: 'Linus Torvalds',
      email: 'torvalds@linux-foundation.org',
      time: 1_756_000_000,
      summary: `commit number ${i}`,
      body: '',
    })),
    repo_refs: [],
    repo_stashes: [],
    graph_rewalk: null,
    repo_submodules: [],
    repo_status: { entries: [], conflicted: [] },
    repo_operation: {
      state: 'clean',
      labels: { ours: 'ours', theirs: 'theirs', swapped: false },
      progress: null,
      headName: null,
      stoppedAt: null,
      interactive: false,
      resumable: true,
    },
    repo_conflicts: [],
    hosting_status: { host: null, detail: 'no remotes', token: 'none' },
    hosting_pull_requests: [],
    remote_list: [],
    // The whole message, which is what rewording has to start from: a commit with a body is
    // the normal case, and the summary alone is not the message.
    commit_detail: {
      commit: {
        oid: 'a'.repeat(40),
        parents: [],
        author: { name: 'Ada', email: 'ada@example.com', time: 0, offset: 0 },
        committer: { name: 'Ada', email: 'ada@example.com', time: 0, offset: 0 },
        summary: 'core: the summary',
        body: 'Why it was done.\n\nSigned-off-by: Ada <ada@example.com>',
      },
      files: [],
    },
    watch_repo: { complete: true, detail: null },
    unwatch_repo: null,
    session_get: SESSION,
    tab_open: SESSION,
    tab_activate: SESSION,
    repo_action: { what: 'done', conflicted: false, message: '' },
    // Where the revision the menu is about stands relative to the current branch. `ahead` is
    // the case that offers everything; the tests about the other three say so.
    rev_ancestry: 'ahead',
  };
}

async function shell(over: Record<string, unknown> = {}, awaitRefs = false) {
  const table = { ...answers(), ...over };
  invoke.mockImplementation(async (cmd: string) => {
    if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
    return table[cmd];
  });
  const view = render(App);
  await waitFor(() => {
    if (view.container.querySelectorAll('li.row').length === 0) throw new Error('no rows yet');
  });
  // The rows arrive with the frame; the refs are a second read and land after it. A menu built
  // in between knows about no refs at all, so wait for a label to be drawn on a row. Waiting
  // on the ref's name in the page is not the same thing: the checked-out branch is in the
  // status bar from the first paint, and a long name is elided in the pill.
  if (awaitRefs) {
    await waitFor(() => {
      if (!view.container.querySelector('.pill-text')) throw new Error('no refs yet');
    });
  }
  return view;
}


describe('finding a commit', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  /** What the details panel was last asked about, which is what the window has landed on. */
  function landedOn(): string | undefined {
    const call = invoke.mock.calls.filter(([cmd]) => cmd === 'commit_detail').at(-1);
    return (call?.[1] as { rev?: string } | undefined)?.rev;
  }

  it('shows the first match rather than counting one nobody can see', async () => {
    // The field said "1 of 500" with the reader left where they were, and stepping forward
    // from it went to the second: the first was reachable only by going round every match.
    const { container } = await shell({
      search_commits: [
        { oid: frameOids[3], row: 3 },
        { oid: frameOids[5], row: 5 },
      ],
    });

    const field = container.querySelector('.find input') as HTMLInputElement | null;
    if (field === null) {
      // The bar opens on Ctrl+F; the shortcut is the only way in.
      await fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    }
    await waitFor(() => {
      if (!container.querySelector('.find input')) throw new Error('no search field yet');
    });
    const input = container.querySelector('.find input') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: 'parser' } });

    await waitFor(
      () => {
        if (landedOn() !== frameOids[3]) throw new Error('not on the first match yet');
      },
      { timeout: 3000 },
    );
  });
});
