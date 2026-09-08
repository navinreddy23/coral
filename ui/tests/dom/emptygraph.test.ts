// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * A repository with nothing in it yet.
 *
 * This is the first screen anybody sees of a repository they have just created, and it was a
 * blank pane beside a panel inviting them to select a commit that does not exist.
 */
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import App from '../../src/app/App.svelte';
import { MAGIC, VERSION } from '../../src/graph/frame';

/** A frame carrying a header and no sections, which is what a walk of nothing produces. */
function emptyFrame(): ArrayBuffer {
  const buffer = new ArrayBuffer(24);
  const view = new DataView(buffer);
  view.setUint32(0, MAGIC, true);
  view.setUint16(4, VERSION, true);
  view.setUint16(6, 0, true); // no sections
  view.setUint32(8, 0, true); // startRow
  view.setUint32(12, 0, true); // rowCount
  view.setUint32(16, 0, true); // totalRows
  view.setUint8(20, 0); // flags
  view.setUint8(21, 20); // hashLen
  return buffer;
}

const REPO = '/new';
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
      head: { kind: 'unborn', name: 'main' },
      state: 'clean',
      commitGraph: false,
    },
    binary_self_test: Uint8Array.from({ length: 4096 }, (_, i) => i % 251).buffer,
    graph_frame: emptyFrame(),
    row_metadata: [],
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
    commit_detail: null,
    watch_repo: { complete: true, detail: null },
    unwatch_repo: null,
    session_get: SESSION,
    tab_open: SESSION,
    tab_activate: SESSION,
    graph_scope: { solo: null, hidden: [] },
    set_graph_scope: null,
  };
}

beforeEach(() => {
  localStorage.clear();
  invoke.mockReset();
});

describe('a repository with no commits', () => {
  it('says so where the graph would be, and does not invite a click on nothing', async () => {
    const table = answers();
    invoke.mockImplementation(async (cmd: string) => {
      if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
      return table[cmd];
    });
    const { container } = render(App);

    await waitFor(() => {
      if (!container.querySelector('.nothing')) throw new Error('no message yet');
    });
    expect(container.querySelector('.nothing')?.textContent).toContain('Nothing is committed yet');
    expect(container.querySelectorAll('li.row')).toHaveLength(0);

    const invite = [...container.querySelectorAll('p.muted')].map((p) => p.textContent ?? '');
    expect(invite.join(' ')).not.toContain('Select a commit');
  });
});
