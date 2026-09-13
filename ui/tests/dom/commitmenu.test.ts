// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * The commit menu, driven through the shell.
 *
 * Four of these items rewrite the branch and one discards uncommitted work, and the only thing
 * between a misclick and a rewritten history is a confirmation. That is what is checked here:
 * not that the engine can drop a commit — `coral-core/tests/rewrite.rs` covers that — but that
 * the window asks first and sends exactly what was asked for.
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

/** Right-clicks the first commit row and returns the menu's labels. */
async function openMenu(container: HTMLElement): Promise<string[]> {
  const row = container.querySelector('li.row') as HTMLElement;
  await fireEvent.contextMenu(row);
  await waitFor(() => {
    if (!container.querySelector('.menu')) throw new Error('no menu');
  });
  return [...container.querySelectorAll('.menu .label')].map((e) => e.textContent?.trim() ?? '');
}

/** Whether the menu item of that name is there but cannot be used. */
function disabledItem(container: HTMLElement, label: string): boolean {
  const button = itemNamed(container, label) as HTMLButtonElement;
  return button.disabled;
}

function itemNamed(container: HTMLElement, label: string): HTMLElement {
  const found = [...container.querySelectorAll('.menu .label')].find(
    (e) => e.textContent?.trim() === label,
  );
  if (!found) throw new Error(`no menu item ${label}`);
  return found.closest('button') as HTMLElement;
}

/** The arguments of the last `repo_action`, which is what every one of these ends in. */
function lastAction(): Record<string, unknown> | undefined {
  const call = invoke.mock.calls.filter(([cmd]) => cmd === 'repo_action').at(-1);
  return (call?.[1] as { action?: Record<string, unknown> } | undefined)?.action;
}

/** Answers the confirmation dialog, if one is up. */
async function confirm(container: HTMLElement, take: boolean): Promise<void> {
  const dialog = await waitFor(() => {
    const found = container.querySelector('[role="dialog"]');
    if (!found) throw new Error('no question yet');
    return found as HTMLElement;
  });
  const buttons = [...dialog.querySelectorAll('button')] as HTMLButtonElement[];
  const cancel = buttons.find((b) => b.className.includes('cancel'));
  // Whichever button answers it, marked primary or marked danger: the most destructive ones
  // are deliberately neither Enter's nor the eye's default.
  const going = buttons.find((b) => b !== cancel);
  const target = take ? going : cancel;
  if (!target) throw new Error(`no ${take ? 'answering' : 'cancel'} button`);
  await fireEvent.click(target);
}

/** A ref on a row, as `repo_refs` places them. Row 0 and one shared commit unless said. */
function on(
  short: string,
  kind: PlacedRef['kind'],
  over: Partial<PlacedRef> = {},
): PlacedRef {
  return {
    name: kind.kind === 'tag' ? `refs/tags/${short}` : `refs/heads/${short}`,
    short,
    kind,
    target: 'a'.repeat(40),
    peeled: null,
    upstream: null,
    ahead: 0,
    behind: 0,
    row: 0,
    ...over,
  };
}

describe('the commit menu', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  it('offers everything the reference does, in its groups', async () => {
    const { container } = await shell();
    const labels = await openMenu(container);

    for (const wanted of [
      'Checkout this commit',
      'Create worktree from this commit',
      'Create branch here',
      'Cherry pick commit…',
      'Revert commit',
      'Edit commit message',
      'Drop commit',
      'Move commit up',
      'Move commit down',
      'Copy commit sha',
      'Copy link to this commit',
      'Create patch from commit',
      'Create tag here',
      'Create annotated tag here',
    ]) {
      expect(labels, wanted).toContain(wanted);
    }
    // The reset submenu names the branch it would move.
    expect(labels.some((l) => l.startsWith('Reset master to'))).toBe(true);
  });

  it('checks a commit out by its own object id, not by its row', async () => {
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout this commit'));

    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'checkout', rev: oidOf(frame, 0) });
    });
  });

  it('moves a commit without asking, since nothing is lost by it', async () => {
    // Reordering is undoable through the journal and destroys nothing, so a confirmation here
    // would only be in the way.
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Move commit up'));

    await waitFor(() => {
      expect(lastAction()).toEqual({
        kind: 'rewrite',
        rev: oidOf(frame, 0),
        how: 'moveNewer',
        message: null,
      });
    });
  });

  it('asks before dropping a commit, and drops it when told to', async () => {
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Drop commit'));

    // Nothing has been sent yet; the question is what stands between a misclick and a
    // rewritten branch.
    expect(lastAction()).toBeUndefined();
    await confirm(container, true);

    await waitFor(() => {
      expect(lastAction()).toEqual({
        kind: 'rewrite',
        rev: oidOf(frame, 0),
        how: 'drop',
        message: null,
      });
    });
  });

  it('drops nothing when the question is dismissed', async () => {
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Drop commit'));
    await confirm(container, false);

    // A moment for anything that was going to be sent.
    await new Promise((r) => setTimeout(r, 20));
    expect(lastAction()).toBeUndefined();
  });

  it('asks before a hard reset, which is the one that discards uncommitted work', async () => {
    const { container } = await shell();
    await openMenu(container);

    const submenu = itemNamed(container, 'Reset master to this commit').closest('.wrap');
    if (submenu) await fireEvent.mouseEnter(submenu);
    await fireEvent.click(itemNamed(container, 'Hard — discard everything since'));

    expect(lastAction()).toBeUndefined();
    await confirm(container, true);
    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'reset', rev: oidOf(frame, 0), mode: 'hard' });
    });
  });

  it('will not let Enter answer the hard reset', async () => {
    // Enter is what people press to make a dialog go away, and the panel's own note says a
    // dialog reading "cannot be recovered" must not be one of them. This is the one button in
    // the window that throws away work nothing can bring back.
    const { container } = await shell();
    await openMenu(container);

    const submenu = itemNamed(container, 'Reset master to this commit').closest('.wrap');
    if (submenu) await fireEvent.mouseEnter(submenu);
    await fireEvent.click(itemNamed(container, 'Hard — discard everything since'));
    await waitFor(() => {
      if (!container.querySelector('[role="dialog"]')) throw new Error('no question yet');
    });

    await fireEvent.keyDown(window, { key: 'Enter' });
    await new Promise((r) => setTimeout(r, 20));
    expect(lastAction()).toBeUndefined();
    // Still up, waiting for an answer rather than dismissed by the same key.
    expect(container.querySelector('[role="dialog"]')).not.toBeNull();

    // Escape is how a dialog goes away, and it takes nothing with it.
    await fireEvent.keyDown(window, { key: 'Escape' });
    await new Promise((r) => setTimeout(r, 20));
    expect(lastAction()).toBeUndefined();
  });

  it('does not ask before a soft reset, which discards nothing', async () => {
    const { container } = await shell();
    await openMenu(container);

    const submenu = itemNamed(container, 'Reset master to this commit').closest('.wrap');
    if (submenu) await fireEvent.mouseEnter(submenu);
    await fireEvent.click(itemNamed(container, 'Soft — keep the index and the working copy'));

    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'reset', rev: oidOf(frame, 0), mode: 'soft' });
    });
  });

  it('offers the whole message when rewording, not just the summary', async () => {
    // The field held the summary and what came back replaced the entire message, so rewording
    // any commit with a body — on a kernel, every commit, with its explanation and its
    // Signed-off-by lines — silently threw the body away.
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Edit commit message'));

    const field = await waitFor(() => {
      const found = container.querySelector('[role="dialog"] textarea');
      if (!found) throw new Error('the question offers no box to type in');
      return found as HTMLTextAreaElement;
    });
    expect(field.value).toBe(
      'core: the summary\n\nWhy it was done.\n\nSigned-off-by: Ada <ada@example.com>',
    );
  });

  it('sends the message it was given when rewording, and nothing when cancelled', async () => {
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Edit commit message'));

    // The field has to be there at all: it was gated on a placeholder being set, so every
    // question that wanted text but had no hint to offer rendered none.
    const field = await waitFor(() => {
      const found = container.querySelector('[role="dialog"] textarea');
      if (!found) throw new Error('the question offers no field to type in');
      return found as HTMLTextAreaElement;
    });
    await fireEvent.input(field, { target: { value: 'core: say it better\n\nand why' } });
    await confirm(container, true);

    await waitFor(() => {
      expect(lastAction()).toEqual({
        kind: 'rewrite',
        rev: oidOf(frame, 0),
        how: 'reword',
        message: 'core: say it better\n\nand why',
      });
    });
  });
});

describe('checking out from the graph', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  it('checks a branch out by its name, not by the commit it happens to be on', async () => {
    // The bug: every checkout from the graph ran `git checkout <oid>`, which detaches HEAD
    // however many branches were sitting on that row.
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );

    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));

    expect(lastAction()).toEqual({ kind: 'checkout', rev: 'topic' });
  });

  it('still offers the commit itself, and says that it detaches', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );

    const labels = await openMenu(container);
    expect(labels).toContain('Checkout topic');
    expect(labels).toContain('Checkout this commit');
    expect(container.textContent).toContain('detached');
  });

  it('does not offer to check out the branch already checked out', async () => {
    // `open_repo` says HEAD is on `master` in this harness.
    const { container } = await shell(
      {
        repo_refs: [on('master', { kind: 'local_branch' }), on('topic', { kind: 'local_branch' })],
      },
      true,
    );

    const labels = await openMenu(container);
    expect(labels).not.toContain('Checkout master');
    expect(labels).toContain('Checkout topic');
  });

  it('checks a tracking branch out under its own name', async () => {
    // `git checkout topic` where only `origin/topic` exists makes a local branch that follows
    // it; `git checkout origin/topic` detaches, which is not what anyone means by clicking it.
    const { container } = await shell(
      { repo_refs: [on('origin/topic', { kind: 'remote_branch', remote: 'origin' })] },
      true,
    );

    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));

    expect(lastAction()).toEqual({ kind: 'checkout', rev: 'topic' });
  });

  it('offers a tracking branch once, when the local branch is there too', async () => {
    const { container } = await shell(
      {
        repo_refs: [
          on('topic', { kind: 'local_branch' }),
          on('origin/topic', { kind: 'remote_branch', remote: 'origin' }),
        ],
      },
      true,
    );

    const labels = await openMenu(container);
    expect(labels.filter((l) => l === 'Checkout topic')).toHaveLength(1);
  });

  it('says a tag detaches, because it does', async () => {
    const { container } = await shell(
      { repo_refs: [on('v1.2.0', { kind: 'tag', annotated: false })] },
      true,
    );

    const labels = await openMenu(container);
    expect(labels).toContain('Checkout v1.2.0');
    expect(container.textContent).toContain('detaches HEAD');
  });

  it('offers nothing extra on a row with no refs on it', async () => {
    const { container } = await shell();
    const labels = await openMenu(container);
    expect(labels.filter((l) => l !== 'Checkout this commit' && l.startsWith('Checkout'))).toEqual(
      [],
    );
    expect(labels).toContain('Checkout this commit');
  });
});

describe('the branch and tag menu', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  function on(short: string, kind: PlacedRef['kind']): PlacedRef {
    return {
      name: kind.kind === 'tag' ? `refs/tags/${short}` : `refs/heads/${short}`,
      short,
      kind,
      target: 'a'.repeat(40),
      peeled: null,
      upstream: null,
      ahead: 0,
      behind: 0,
      row: 0,
    };
  }

  /** Opens a collapsed section of the panel, so its rows exist to be clicked. */
  async function expand(container: HTMLElement, title: string) {
    const head = [...container.querySelectorAll('.head')].find((e) =>
      e.textContent?.includes(title),
    );
    if (head) await fireEvent.click(head);
  }

  /** Opens the menu at a ref's dots in the panel that lists them. */
  async function refMenu(container: HTMLElement, short: string): Promise<string[]> {
    const dots = await waitFor(() => {
      const found = container.querySelector(`[title="What can be done with ${short}"]`);
      if (!found) {
        const titles = [...container.querySelectorAll('[title]')].map((e) => e.getAttribute('title'));
        throw new Error(`no dots for ${short}; have ${JSON.stringify(titles)}`);
      }
      return found as HTMLElement;
    });
    await fireEvent.click(dots);
    await waitFor(() => {
      if (!container.querySelector('.menu')) throw new Error('no menu');
    });
    return [...container.querySelectorAll('.menu .label')].map((e) => e.textContent?.trim() ?? '');
  }

  it('checks a tag out from the panel that lists it', async () => {
    // A tag is a place in the history like any other; the panel listed them and offered
    // nothing to do with one.
    const { container } = await shell(
      { repo_refs: [on('v1.2.0', { kind: 'tag', annotated: false })] },
      true,
    );

    await expand(container, 'Tags');
    const labels = await refMenu(container, 'v1.2.0');
    expect(labels).toContain('Checkout v1.2.0');

    await fireEvent.click(itemNamed(container, 'Checkout v1.2.0'));
    expect(lastAction()).toEqual({ kind: 'checkout', rev: 'v1.2.0' });
  });

  it('offers to push a tag, since a push sends none of them', async () => {
    // Tags do not travel with a push. A tag made in the window sat there looking published
    // and was on nobody else's machine.
    const { container } = await shell(
      {
        repo_refs: [on('v1.2.0', { kind: 'tag', annotated: false })],
        remote_list: [
          { name: 'origin', fetchUrl: 'git@example.com:x.git', pushUrl: 'git@example.com:x.git' },
        ],
      },
      true,
    );

    // The remotes are read without being waited on, so the window can open before they land.
    await waitFor(() => {
      if (!invoke.mock.calls.some(([cmd]) => cmd === 'remote_list')) throw new Error('not yet');
    });
    await new Promise((r) => setTimeout(r, 10));
    await expand(container, 'Tags');
    const labels = await refMenu(container, 'v1.2.0');
    expect(labels).toContain('Push v1.2.0 to origin');

    await fireEvent.click(itemNamed(container, 'Push v1.2.0 to origin'));
    await waitFor(() => {
      expect(lastAction()).toEqual({
        kind: 'push',
        remote: 'origin',
        setUpstream: false,
        refspec: 'refs/tags/v1.2.0',
        tags: false,
        forceWithLease: false,
        delete: false,
      });
    });
  });

  /**
   * A branch that is not checked out had no way to reach a remote at all. Somebody with three
   * topic branches ahead of origin had to check each one out to send it, and the toolbar's
   * Push only ever means the branch you are standing on.
   */
  it('offers to push a branch that is not the one checked out', async () => {
    const { container } = await shell(
      {
        repo_refs: [
          on('main', { kind: 'local_branch' }),
          on('topic', { kind: 'local_branch' }),
        ],
        repo_head: { name: 'main', detached: false },
        remote_list: [
          { name: 'origin', fetchUrl: 'git@example.com:x.git', pushUrl: 'git@example.com:x.git' },
        ],
      },
      true,
    );
    await waitFor(() => {
      if (!invoke.mock.calls.some(([cmd]) => cmd === 'remote_list')) throw new Error('not yet');
    });
    await new Promise((r) => setTimeout(r, 10));

    const labels = await refMenu(container, 'topic');
    expect(labels).toContain('Push topic to origin');

    await fireEvent.click(itemNamed(container, 'Push topic to origin'));
    await waitFor(() => {
      expect(lastAction()).toEqual({
        kind: 'push',
        remote: 'origin',
        setUpstream: true,
        refspec: 'refs/heads/topic',
        tags: false,
        forceWithLease: false,
        delete: false,
      });
    });
  });

  it('offers no push for a branch when there is no remote to push it to', async () => {
    const { container } = await shell(
      {
        repo_refs: [on('main', { kind: 'local_branch' })],
        repo_head: { name: 'main', detached: false },
        remote_list: [],
      },
      true,
    );
    const labels = await refMenu(container, 'main');
    expect(labels.some((l) => l.startsWith('Push main'))).toBe(false);
  });

  it('offers no push for a tag when there is no remote to push it to', async () => {
    const { container } = await shell(
      { repo_refs: [on('v1.2.0', { kind: 'tag', annotated: false })], remote_list: [] },
      true,
    );
    await expand(container, 'Tags');
    const labels = await refMenu(container, 'v1.2.0');
    expect(labels.some((l) => l.startsWith('Push v1.2.0'))).toBe(false);
  });

  it('fast-forwards a tag the branch has passed, and never goes back to it', async () => {
    // The reported bug. On a master that is up to date, right-clicking a release tag read
    // "Fast-forward master to v1.0.0", which git refuses: master is the one in front. The
    // direction that means anything here is the other one.
    const { container } = await shell(
      {
        repo_refs: [on('v1.0.0', { kind: 'tag', annotated: false })],
        rev_ancestry: 'behind',
      },
      true,
    );

    await expand(container, 'Tags');
    const labels = await refMenu(container, 'v1.0.0');
    expect(labels).not.toContain('Fast-forward master to v1.0.0');
    expect(labels).toContain('Fast-forward v1.0.0 to master…');
  });

  it('keeps the rest of the group whichever way the tag stands', async () => {
    // Only the fast-forward changes direction. Merging or rebasing onto something the branch
    // already contains is a no-op git states plainly, and `rebase -i` onto an ancestor is not
    // a no-op at all — it is how anybody edits the history since their last release. Removing
    // them left a menu with one line in it.
    const { container } = await shell(
      {
        repo_refs: [on('v1.0.0', { kind: 'tag', annotated: false })],
        rev_ancestry: 'behind',
      },
      true,
    );
    await expand(container, 'Tags');
    const labels = await refMenu(container, 'v1.0.0');
    expect(labels).toContain('Merge v1.0.0 into master');
    expect(labels).toContain('Rebase master onto v1.0.0');
    expect(labels).toContain('Rebase master onto v1.0.0, interactively');
  });

  it('asks before fast-forwarding a tag, and moves it when told to', async () => {
    // A tag that has been fetched anywhere else does not come back, so this is not a click
    // away from being done.
    const { container } = await shell(
      {
        repo_refs: [on('v1.0.0', { kind: 'tag', annotated: false })],
        rev_ancestry: 'behind',
      },
      true,
    );
    await expand(container, 'Tags');
    await refMenu(container, 'v1.0.0');
    await fireEvent.click(itemNamed(container, 'Fast-forward v1.0.0 to master…'));

    await confirm(container, true);
    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'tagMove', name: 'v1.0.0', at: 'master' });
    });
  });

  it('moves no tag when the question is dismissed', async () => {
    const { container } = await shell(
      {
        repo_refs: [on('v1.0.0', { kind: 'tag', annotated: false })],
        rev_ancestry: 'behind',
      },
      true,
    );
    await expand(container, 'Tags');
    await refMenu(container, 'v1.0.0');
    await fireEvent.click(itemNamed(container, 'Fast-forward v1.0.0 to master…'));

    await confirm(container, false);
    expect(lastAction()).toBeUndefined();
  });

  it('says nothing is lost when deleting a branch this one contains', async () => {
    // Coral deletes with `-D`, so git never refuses and this question is the only thing
    // between a misclick and an orphaned commit. Saying the same alarming sentence for a
    // merged branch is a warning nobody reads by the time it matters.
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })], rev_ancestry: 'behind' },
      true,
    );
    await refMenu(container, 'topic');
    await fireEvent.click(itemNamed(container, 'Delete topic…'));

    const dialog = await waitFor(() => {
      const found = container.querySelector('[role="dialog"]');
      if (!found) throw new Error('no question yet');
      return found as HTMLElement;
    });
    expect(dialog.textContent).toContain('nothing is lost');
    expect(dialog.textContent).not.toContain('effectively gone');
  });

  it('warns when deleting a branch holding commits this one does not', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })], rev_ancestry: 'diverged' },
      true,
    );
    await refMenu(container, 'topic');
    await fireEvent.click(itemNamed(container, 'Delete topic…'));

    const dialog = await waitFor(() => {
      const found = container.querySelector('[role="dialog"]');
      if (!found) throw new Error('no question yet');
      return found as HTMLElement;
    });
    expect(dialog.textContent).toContain('effectively gone');
  });

  it('fast-forwards a branch that has fallen behind, without checking it out', async () => {
    // The same question for a branch, where the answer is a real fast-forward rather than a
    // replacement, and needs no checkout.
    const { container } = await shell(
      { repo_refs: [on('behind', { kind: 'local_branch' })], rev_ancestry: 'behind' },
      true,
    );

    const labels = await refMenu(container, 'behind');
    expect(labels).toContain('Fast-forward behind to master');
    expect(labels).not.toContain('Fast-forward master to behind');

    await fireEvent.click(itemNamed(container, 'Fast-forward behind to master'));
    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'branchFastForward', name: 'behind', at: 'master' });
    });
  });

  it('offers what can be done with a branch, not only going to it', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );

    const labels = await refMenu(container, 'topic');
    expect(labels).toContain('Checkout topic');
    expect(labels).toContain('Merge topic into master');
    expect(labels).toContain('Rebase master onto topic');
    expect(labels).toContain('Cherry pick commit…');
    expect(labels).toContain('Delete topic…');
  });

  it('will not offer to delete or check out the branch already checked out', async () => {
    const { container } = await shell(
      { repo_refs: [on('master', { kind: 'local_branch' })] },
      true,
    );

    const labels = await refMenu(container, 'master');
    expect(labels).not.toContain('Checkout master');
    expect(labels).not.toContain('Delete master…');
    expect(labels).not.toContain('Merge master into master');
  });

  it('merges the ref that was asked about', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );

    await refMenu(container, 'topic');
    await fireEvent.click(itemNamed(container, 'Merge topic into master'));
    expect(lastAction()).toEqual({ kind: 'merge', rev: 'topic', mode: 'auto' });
  });

  it('rebases onto the ref that was asked about', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );

    await refMenu(container, 'topic');
    await fireEvent.click(itemNamed(container, 'Rebase master onto topic'));
    expect(lastAction()).toEqual({ kind: 'rebase', onto: 'topic' });
  });
});

describe('cherry picking', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  it('asks whether to commit, and records one when the answer is yes', async () => {
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Cherry pick commit…'));

    await waitFor(() => {
      if (!container.textContent?.includes('Commit the cherry picked changes?')) {
        throw new Error('no question yet');
      }
    });
    await confirm(container, true);

    await waitFor(() => {
      if (lastAction() === undefined) throw new Error('not yet');
    });
    expect(lastAction()).toMatchObject({ kind: 'cherryPick', commit: true });
  });

  it('leaves the changes staged when the answer is no', async () => {
    // For someone who wants to change it, split it, or fold it into something else before
    // anything is recorded.
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Cherry pick commit…'));

    const dialog = await waitFor(() => {
      const found = container.querySelector('[role="dialog"]');
      if (!found) throw new Error('no question yet');
      return found as HTMLElement;
    });
    const no = [...dialog.querySelectorAll('button')].find((b) => b.textContent?.trim() === 'No');
    await fireEvent.click(no as HTMLElement);

    await waitFor(() => {
      if (lastAction() === undefined) throw new Error('not yet');
    });
    expect(lastAction()).toMatchObject({ kind: 'cherryPick', commit: false });
  });

  it('does nothing at all when the question is dismissed', async () => {
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Cherry pick commit…'));
    await confirm(container, false);

    expect(lastAction()).toBeUndefined();
  });
});

/**
 * Bringing a row into the current branch, from the commit menu.
 *
 * The branch panel has offered these since the beginning; the graph did not, and the graph is
 * where people right-click. Each sends exactly one action, named for what the row carries.
 */
describe('merging and rebasing from the graph', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  it('offers all four, named for the ref on the row', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );

    const labels = await openMenu(container);
    expect(labels).toContain('Fast-forward master to topic');
    expect(labels).toContain('Merge topic into master');
    expect(labels).toContain('Rebase master onto topic');
    expect(labels).toContain('Rebase master onto topic, interactively');
  });

  it('fast-forwards with the mode that refuses to merge', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Fast-forward master to topic'));

    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'merge', rev: 'topic', mode: 'ffOnly' });
    });
  });

  it('merges the ref, not the commit under it', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Merge topic into master'));

    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'merge', rev: 'topic', mode: 'auto' });
    });
  });

  it('rebases onto the ref', async () => {
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })] },
      true,
    );
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Rebase master onto topic'));

    await waitFor(() => {
      expect(lastAction()).toEqual({ kind: 'rebase', onto: 'topic' });
    });
  });

  it('names a row with no ref by its commit', async () => {
    const { container } = await shell();
    const labels = await openMenu(container);
    const short = oidOf(frame, 0).slice(0, 8);
    expect(labels).toContain(`Merge ${short} into master`);
    expect(labels).toContain(`Rebase master onto ${short}`);
  });

  it('offers none of it on the row the branch is already on', async () => {
    // Merging a branch into itself is a no-op, and rebasing it onto itself is worse than one.
    // Decided by asking git where the commit stands, not by matching the label on the row: a
    // second branch sitting on the same commit is the same no-op under another name.
    const { container } = await shell(
      { repo_refs: [on('master', { kind: 'local_branch' })], rev_ancestry: 'same' },
      true,
    );
    const labels = await openMenu(container);
    expect(labels.some((l) => l.startsWith('Merge '))).toBe(false);
    expect(labels.some((l) => l.startsWith('Rebase master onto'))).toBe(false);
  });

  it('says why rather than vanishing when a bare commit cannot be fast-forwarded to', async () => {
    // A commit row carries no ref to move, so there is no other direction to offer. The line
    // stays where it is, disabled, saying which way round they are: it used to read
    // "Fast-forward master to <sha>", which git refuses because master is in front.
    const { container } = await shell({ rev_ancestry: 'behind' });
    const labels = await openMenu(container);
    const short = oidOf(frame, 0).slice(0, 8);
    expect(labels).toContain(`Fast-forward master to ${short}`);
    expect(disabledItem(container, `Fast-forward master to ${short}`)).toBe(true);
    expect(labels).toContain(`Rebase master onto ${short}, interactively`);
  });

  it('disables the fast-forward when the two have diverged, and keeps the rest', async () => {
    // git refuses a fast-forward that would lose commits, so it cannot be run — but a line
    // that disappears teaches nobody why. A merge or a rebase is what this case is for.
    const { container } = await shell(
      { repo_refs: [on('topic', { kind: 'local_branch' })], rev_ancestry: 'diverged' },
      true,
    );
    const labels = await openMenu(container);
    expect(labels).toContain('Fast-forward master to topic');
    expect(disabledItem(container, 'Fast-forward master to topic')).toBe(true);
    expect(labels).toContain('Merge topic into master');
    expect(labels).toContain('Rebase master onto topic');
  });
});

/**
 * Checking out a remote branch when a local branch of that name already exists.
 *
 * `git checkout topic` lands on the local branch wherever it happens to be, which is not what
 * asking for `origin/topic` looks like it does. GitKraken asks; so does this.
 */
describe('a remote branch with a local of its own name', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  /** `origin/topic` on the row under the pointer, `topic` elsewhere and on another commit. */
  const diverged = [
    on('origin/topic', { kind: 'remote_branch', remote: 'origin' }, {
      name: 'refs/remotes/origin/topic',
      target: 'b'.repeat(40),
    }),
    on('topic', { kind: 'local_branch' }, { row: 3, upstream: 'origin/topic', ahead: 2, behind: 1 }),
  ];

  /** Clicks the dialog button whose label matches. */
  async function answer(container: HTMLElement, label: string) {
    const dialog = await waitFor(() => {
      const found = container.querySelector('[role="dialog"]');
      if (!found) throw new Error('no question yet');
      return found as HTMLElement;
    });
    const button = [...dialog.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === label,
    );
    if (!button) throw new Error(`no button ${label}`);
    await fireEvent.click(button);
  }

  /** Every `repo_action` sent, oldest first. */
  function actions(): Record<string, unknown>[] {
    return invoke.mock.calls
      .filter(([cmd]) => cmd === 'repo_action')
      .map(([, args]) => (args as { action: Record<string, unknown> }).action);
  }

  it('still offers the remote branch, though a local of that name exists elsewhere', async () => {
    const { container } = await shell({ repo_refs: diverged }, true);
    const labels = await openMenu(container);
    expect(labels).toContain('Checkout topic');
  });

  it('asks rather than quietly checking out the local one', async () => {
    const { container } = await shell({ repo_refs: diverged }, true);
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));

    const dialog = await waitFor(() => {
      const found = container.querySelector('[role="dialog"]');
      if (!found) throw new Error('no question yet');
      return found as HTMLElement;
    });
    expect(dialog.textContent).toContain('origin/topic');
    expect(actions()).toHaveLength(0);
  });

  it('checks out the local branch as it stands, when that is the answer', async () => {
    const { container } = await shell({ repo_refs: diverged }, true);
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));
    await answer(container, 'Checkout topic');

    await waitFor(() => {
      if (actions().length === 0) throw new Error('nothing sent');
    });
    expect(actions()).toEqual([{ kind: 'checkout', rev: 'topic' }]);
  });

  it('resets the local branch onto the remote, when that is the answer', async () => {
    const { container } = await shell({ repo_refs: diverged }, true);
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));
    await answer(container, 'Reset topic to origin/topic');

    // In that order: the reset moves whatever HEAD is on, so it must follow a checkout that
    // worked. Reversed, or run regardless, it rewrites the branch the user was standing on.
    await waitFor(() => {
      if (actions().length < 2) throw new Error('not yet');
    });
    expect(actions()).toEqual([
      { kind: 'checkout', rev: 'topic' },
      { kind: 'reset', rev: 'origin/topic', mode: 'hard' },
    ]);
  });

  it('resets nothing when the checkout fails', async () => {
    const { container } = await shell(
      { repo_refs: diverged, repo_action: null },
      true,
    );
    // A failing checkout answers with an error, and the branch must be left where it is.
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'repo_action') throw { code: 'git', message: 'local changes would be lost' };
      const table: Record<string, unknown> = { ...answers(), repo_refs: diverged };
      if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
      return table[cmd];
    });
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));
    await answer(container, 'Reset topic to origin/topic');

    await waitFor(() => {
      if (actions().length === 0) throw new Error('nothing sent');
    });
    await new Promise((r) => setTimeout(r, 20));
    expect(actions()).toEqual([{ kind: 'checkout', rev: 'topic' }]);
  });

  it('does not ask when the two are on the same commit', async () => {
    const { container } = await shell(
      {
        repo_refs: [
          on('topic', { kind: 'local_branch' }),
          on('origin/topic', { kind: 'remote_branch', remote: 'origin' }),
        ],
      },
      true,
    );
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));

    await waitFor(() => {
      if (actions().length === 0) throw new Error('nothing sent');
    });
    expect(container.querySelector('[role="dialog"]')).toBeNull();
    expect(actions()).toEqual([{ kind: 'checkout', rev: 'topic' }]);
  });
});

/**
 * `docs/ui-spec.md` says a double-click on a branch pill checks it out, the way every client
 * this one is meant to feel like does. Nothing did it: both clicks landed on the row.
 */
describe('double-clicking a pill', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  function on(short: string, kind: PlacedRef['kind']): PlacedRef {
    return {
      name: kind.kind === 'tag' ? `refs/tags/${short}` : `refs/heads/${short}`,
      short,
      kind,
      target: 'a'.repeat(40),
      peeled: null,
      upstream: null,
      ahead: 0,
      behind: 0,
      row: 0,
    };
  }

  /** The pill drawn on the first row, which is the one the fixture's refs sit on. */
  function pill(container: HTMLElement): HTMLElement {
    const found = container.querySelector('button.pill');
    if (!found) throw new Error('no pill on the row');
    return found as HTMLElement;
  }

  function actions(): Record<string, unknown>[] {
    return invoke.mock.calls
      .filter(([cmd]) => cmd === 'repo_action')
      .map(([, args]) => (args as { action: Record<string, unknown> }).action);
  }

  it('checks out the branch it names', async () => {
    const { container } = await shell({ repo_refs: [on('topic', { kind: 'local_branch' })] }, true);
    await fireEvent.dblClick(pill(container));

    await waitFor(() => {
      if (actions().length === 0) throw new Error('nothing sent');
    });
    expect(actions()).toEqual([{ kind: 'checkout', rev: 'topic' }]);
  });

  it('takes a tracking branch by its own name, as the menu does', async () => {
    const { container } = await shell(
      { repo_refs: [on('origin/topic', { kind: 'remote_branch', remote: 'origin' })] },
      true,
    );
    await fireEvent.dblClick(pill(container));

    await waitFor(() => {
      if (actions().length === 0) throw new Error('nothing sent');
    });
    expect(actions()).toEqual([{ kind: 'checkout', rev: 'topic' }]);
  });

  it('leaves a tag alone, because checking one out detaches HEAD', async () => {
    const { container } = await shell(
      { repo_refs: [on('v1.2.0', { kind: 'tag', annotated: false })] },
      true,
    );
    await fireEvent.dblClick(pill(container));

    await new Promise((r) => setTimeout(r, 20));
    expect(actions()).toEqual([]);
  });

  it('does nothing on the branch already checked out', async () => {
    // `open_repo` says HEAD is on `master` in this harness.
    const { container } = await shell(
      { repo_refs: [on('master', { kind: 'local_branch' })] },
      true,
    );
    await fireEvent.dblClick(pill(container));

    await new Promise((r) => setTimeout(r, 20));
    expect(actions()).toEqual([]);
  });

  it('does nothing in a bare repository, which has nothing to check out into', async () => {
    const { container } = await shell(
      {
        repo_refs: [on('topic', { kind: 'local_branch' })],
        open_repo: {
          path: '/repo',
          gitDir: '/repo',
          gitVersion: '2.43.0',
          isBare: true,
          head: { kind: 'branch', name: 'master' },
          state: 'clean',
          commitGraph: true,
        },
      },
      true,
    );
    await fireEvent.dblClick(pill(container));

    await new Promise((r) => setTimeout(r, 20));
    expect(actions()).toEqual([]);
  });
});

/**
 * Undo and redo describe a file beside the repository, and every action can move it.
 *
 * The buttons carry the name of what they would act on. Read once and left, that name was the
 * one from two actions ago: the tooltip said "create branch topic/checkme" and the button
 * undid a checkout.
 */
describe('what undo and redo say they will do', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  function on(short: string, kind: PlacedRef['kind']): PlacedRef {
    return {
      name: kind.kind === 'tag' ? `refs/tags/${short}` : `refs/heads/${short}`,
      short,
      kind,
      target: 'a'.repeat(40),
      peeled: null,
      upstream: null,
      ahead: 0,
      behind: 0,
      row: 0,
    };
  }

  it('is read again after an action that could have moved it', async () => {
    const { container } = await shell({ repo_refs: [on('topic', { kind: 'local_branch' })] }, true);
    const readsBefore = invoke.mock.calls.filter(([cmd]) => cmd === 'repo_journal').length;

    await fireEvent.dblClick(container.querySelector('button.pill') as HTMLElement);
    await waitFor(() => {
      const now = invoke.mock.calls.filter(([cmd]) => cmd === 'repo_journal').length;
      if (now <= readsBefore) throw new Error('the journal was not read again');
    });
  });
});

/**
 * A stash has a row in the graph like anything else, and the menu on it was the commit menu:
 * drop the commit, move it up, edit its message, rebase from it. None of that means anything
 * for a stash, and "Drop commit" beside them reads as the one thing that does.
 */
describe('the menu on a stash row', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  const stash = {
    index: 0,
    name: 'stash@{0}',
    oid: frameOids[0],
    message: 'WIP on main',
    branch: 'main',
    time: 1_756_000_000,
    row: 0,
  };

  it('offers what a stash can do, and nothing that rewrites the branch', async () => {
    const { container } = await shell({ repo_stashes: [stash] });
    // The stack is a second read that lands after the rows; a menu opened in between knows of
    // no stash at all. The panel's own section is what says it has arrived.
    await waitFor(() => {
      if (!container.textContent?.includes('WIP on main')) throw new Error('no stash yet');
    });
    const labels = await openMenu(container);

    expect(labels).toContain('Apply it, and keep it');
    expect(labels).toContain('Pop it');
    expect(labels).toContain('Drop it…');
    for (const gone of [
      'Drop commit',
      'Move commit up',
      'Move commit down',
      'Edit commit message',
      'Interactive rebase from this commit',
      'Revert commit',
    ]) {
      expect(labels).not.toContain(gone);
    }
  });

  it('still gives an ordinary commit its own menu', async () => {
    // The row the stash is on is the only one that changes.
    const { container } = await shell({ repo_stashes: [{ ...stash, oid: 'f'.repeat(40), row: 5 }] });
    await waitFor(() => {
      if (!container.querySelector('li.row')) throw new Error('no rows yet');
    });
    const labels = await openMenu(container);
    expect(labels).toContain('Drop commit');
    expect(labels).not.toContain('Pop it');
  });
});
