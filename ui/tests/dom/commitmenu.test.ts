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
    },
    repo_conflicts: [],
    hosting_status: { host: null, detail: 'no remotes', signedIn: false },
    hosting_pull_requests: [],
    remote_list: [],
    commit_detail: null,
    watch_repo: { complete: true, detail: null },
    unwatch_repo: null,
    session_get: SESSION,
    tab_open: SESSION,
    tab_activate: SESSION,
    repo_action: { what: 'done', conflicted: false, message: '' },
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
  const primary = buttons.find((b) => b.className.includes('primary'));
  const cancel = buttons.find((b) => b.className.includes('cancel'));
  const target = take ? primary : cancel;
  if (!target) throw new Error(`no ${take ? 'primary' : 'cancel'} button`);
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

  it('sends the message it was given when rewording, and nothing when cancelled', async () => {
    const { container } = await shell();
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Edit commit message'));

    // The field has to be there at all: it was gated on a placeholder being set, so every
    // question that wanted text but had no hint to offer rendered none.
    const field = await waitFor(() => {
      const found = container.querySelector('[role="dialog"] input');
      if (!found) throw new Error('the question offers no field to type in');
      return found as HTMLInputElement;
    });
    await fireEvent.input(field, { target: { value: 'core: say it better' } });
    await confirm(container, true);

    await waitFor(() => {
      expect(lastAction()).toEqual({
        kind: 'rewrite',
        rev: oidOf(frame, 0),
        how: 'reword',
        message: 'core: say it better',
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
    const { container } = await shell(
      { repo_refs: [on('master', { kind: 'local_branch' })] },
      true,
    );
    const labels = await openMenu(container);
    expect(labels.some((l) => l.startsWith('Merge '))).toBe(false);
    expect(labels.some((l) => l.startsWith('Rebase master onto'))).toBe(false);
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
    const { container } = await shell({ repo_refs: diverged }, 'origin/topic');
    const labels = await openMenu(container);
    expect(labels).toContain('Checkout topic');
  });

  it('asks rather than quietly checking out the local one', async () => {
    const { container } = await shell({ repo_refs: diverged }, 'origin/topic');
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
    const { container } = await shell({ repo_refs: diverged }, 'origin/topic');
    await openMenu(container);
    await fireEvent.click(itemNamed(container, 'Checkout topic'));
    await answer(container, 'Checkout topic');

    await waitFor(() => {
      if (actions().length === 0) throw new Error('nothing sent');
    });
    expect(actions()).toEqual([{ kind: 'checkout', rev: 'topic' }]);
  });

  it('resets the local branch onto the remote, when that is the answer', async () => {
    const { container } = await shell({ repo_refs: diverged }, 'origin/topic');
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
      const table = { ...answers(), repo_refs: diverged };
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
