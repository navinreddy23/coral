// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import Staging from '../../src/app/Staging.svelte';
import { CommitState } from '../../src/state/commit.svelte';
import { WorktreeState } from '../../src/state/worktree.svelte';
import type { Status, StatusEntry } from '../../src/ipc/types';

function entry(path: string, over: Partial<StatusEntry> = {}): StatusEntry {
  return {
    path,
    origPath: null,
    index: 'unmodified',
    worktree: 'modified',
    conflict: null,
    score: null,
    submodule: false,
    ...over,
  };
}

/** Three unstaged files across two directories, and one staged. */
function status(): Status {
  return {
    entries: [
      entry('src/app/App.svelte'),
      entry('src/state/graph.ts', { worktree: 'untracked' }),
      entry('docs/ARCHITECTURE.md'),
      entry('build-log.txt', { index: 'added', worktree: 'unmodified' }),
    ],
    branch: 'main',
    upstream: null,
    ahead: 0,
    behind: 0,
  } as unknown as Status;
}

async function panel(over: Record<string, unknown> = {}) {
  const worktree = new WorktreeState();
  // The message being written is the window's, not the panel's: the panel is unmounted
  // whenever the selection leaves the working copy, and a half-written message survives that.
  const commit = new CommitState();
  // Through `load`, not by assignment: staging is refused until the state knows which
  // repository it is acting on, which is what a bare assignment would skip.
  await worktree.load('/repo');
  const onOpenFile = vi.fn();
  return {
    worktree,
    commit,
    onOpenFile,
    ...render(Staging, {
      props: {
        worktree,
        commit,
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onOpenFile,
        ...over,
      },
    }),
  };
}

/** The heading text of each section, in the order they appear. */
function headings(container: HTMLElement): string[] {
  return [...container.querySelectorAll('section h3')].map((h) =>
    h.textContent?.replace(/\s+/g, ' ').trim() ?? '',
  );
}

describe('the staging panel', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async () => status());
  });

  it('puts unstaged above staged, which is the direction work moves', async () => {
    const { container } = await panel();
    const [first, second] = headings(container);
    expect(first).toContain('Unstaged files (3)');
    expect(second).toContain('Staged files (1)');
  });

  it('counts every change in the header, and names the branch they are on', async () => {
    const { container } = await panel();
    const header = (container.querySelector('header')?.textContent ?? '').replace(/\s+/g, ' ');
    expect(header).toContain('4 file changes');
    expect(header).toContain('main');
  });

  it('shows a directory row with what is inside it, without being opened', async () => {
    // A folder that only says its name hides exactly the thing the panel exists to show.
    const { container } = await panel();
    const dirs = [...container.querySelectorAll('.dir')].map((d) =>
      d.textContent?.replace(/\s+/g, ' ').trim(),
    );
    // `src/app` and `src/state` hold one file each, so the run collapses to `src` holding two.
    expect(dirs.some((d) => d?.startsWith('⌄ src'))).toBe(true);
    expect(dirs.some((d) => d?.includes('✎ 1') && d.includes('+ 1'))).toBe(true);
  });

  it('opens the diff for the half the file was clicked in', async () => {
    // The same file can be in both lists with different hunks, so which side matters.
    const { container, onOpenFile } = await panel();
    const staged = [...container.querySelectorAll('section')][1];
    const file = staged?.querySelector('.file') as HTMLElement;
    await fireEvent.click(file);

    expect(onOpenFile).toHaveBeenCalledWith('build-log.txt', true);
  });

  it('stages every file under a directory in one call', async () => {
    const { container } = await panel();
    const dir = [...container.querySelectorAll('.dir')].find((d) =>
      d.textContent?.includes('src'),
    );
    const act = dir?.parentElement?.querySelector('.act') as HTMLElement;
    await fireEvent.click(act);

    const call = invoke.mock.calls.filter(([cmd]) => cmd === 'stage_paths').at(-1);
    expect(call?.[1]).toEqual({
      path: '/repo',
      paths: ['src/app/App.svelte', 'src/state/graph.ts'],
      stage: true,
    });
  });

  it('will not commit without a summary, however much is staged', async () => {
    const { container } = await panel();
    const button = container.querySelector('.commit') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it('joins the summary and the description the way git expects', async () => {
    const { container, worktree } = await panel();
    const commit = vi.spyOn(worktree, 'commit').mockResolvedValue(undefined);

    await fireEvent.input(container.querySelector('.summary') as HTMLInputElement, {
      target: { value: 'app: do the thing' },
    });
    await fireEvent.input(container.querySelector('.description') as HTMLTextAreaElement, {
      target: { value: 'Because the other thing was wrong.' },
    });
    await fireEvent.click(container.querySelector('.commit') as HTMLButtonElement);

    // Subject, blank line, body.
    expect(commit).toHaveBeenCalledWith(
      'app: do the thing\n\nBecause the other thing was wrong.',
      false,
    );
  });

  it('counts down to the summary limit rather than refusing a long one', async () => {
    const { container } = await panel();
    const field = container.querySelector('.summary') as HTMLInputElement;
    await fireEvent.input(field, { target: { value: 'x'.repeat(80) } });

    const limit = container.querySelector('.limit');
    expect(limit?.textContent?.trim()).toBe('-8');
    expect(limit?.className).toContain('over');
    // Still committable: the commit is the user's to write.
    expect((container.querySelector('.commit') as HTMLButtonElement).disabled).toBe(false);
  });

  it('lists conflicts above both, since nothing else can finish until they are settled', () => {
    const worktree = new WorktreeState();
    worktree.status = {
      ...status(),
      entries: [entry('both.txt', { conflict: 'both_modified' }), entry('a.txt')],
    } as Status;
    const { container } = render(Staging, {
      props: {
        worktree,
        commit: new CommitState(),
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onOpenFile: vi.fn(),
      },
    });

    expect(container.querySelector('h3.conflict')?.textContent).toContain('Conflicts (1)');
    const order = [...container.querySelectorAll('h3')].map((h) => h.textContent ?? '');
    expect(order[0]).toContain('Conflicts');
  });

  it('says so when a list is empty rather than showing a bare heading', () => {
    const worktree = new WorktreeState();
    worktree.status = { ...status(), entries: [] } as Status;
    const { container } = render(Staging, {
      props: {
        worktree,
        commit: new CommitState(),
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onOpenFile: vi.fn(),
      },
    });

    const empties = [...container.querySelectorAll('.empty')].map((e) => e.textContent);
    expect(empties).toHaveLength(2);
    expect(empties[1]).toContain('A commit needs something in here');
  });
});

describe('a conflicted file', () => {
  it('is listed under conflicts and nowhere else', async () => {
    // git writes `UU` for a path with both sides in the index, so its index column reads as a
    // change. It is not a staged one: nothing commits until the conflict is settled, and the
    // file was being drawn twice with the commit button counting it.
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd !== 'repo_status') throw new Error(`unstubbed ${cmd}`);
      return {
        ...status(),
        entries: [
          entry('README.md', {
            index: 'modified',
            worktree: 'modified',
            conflict: 'both_modified',
          }),
        ],
      };
    });
    const { container, worktree } = await panel();

    expect(worktree.conflicted.map((e) => e.path)).toEqual(['README.md']);
    expect(worktree.staged, 'not staged as well').toEqual([]);
    expect(worktree.unstaged, 'nor unstaged').toEqual([]);

    const named = [...container.querySelectorAll('.file .name')].map((e) => e.textContent);
    expect(named.filter((n) => n === 'README.md'), 'drawn once').toHaveLength(1);
  });
});
