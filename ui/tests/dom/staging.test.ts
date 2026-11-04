// @vitest-environment happy-dom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { render } from '@testing-library/svelte';
import { fireEvent, waitFor } from '@testing-library/dom';
import { tick } from 'svelte';
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
import { SECTION_LIMITS, SectionsState } from '../../src/state/sections.svelte';
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
    modeChanged: false,
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
        sections: new SectionsState(),
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onDiscard: () => {},
        onFileMenu: () => {},
        onCommit: () => {},
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
    localStorage.clear();
    invoke.mockReset();
    invoke.mockImplementation(async () => status());
  });

  it('says a summary is what is missing, not how many files it would take', async () => {
    // The button already names what is missing when nothing is staged. With something staged
    // and the box above it empty it stayed disabled and read "Commit 1 file", which says
    // nothing about why pressing it does nothing.
    const { container } = await panel();
    const button = container.querySelector('button.commit') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(button.textContent?.replace(/\s+/gu, ' ').trim()).toBe('Write a summary to commit');

    await fireEvent.input(container.querySelector('.summary') as HTMLInputElement, {
      target: { value: 'a summary' },
    });
    expect(button.disabled).toBe(false);
    expect(button.textContent?.replace(/\s+/gu, ' ').trim()).toBe('Commit 1 file');
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
    // The caret is drawn rather than spelled now, so the row's text begins with the name.
    expect(dirs.some((d) => d?.startsWith('src'))).toBe(true);
    expect(container.querySelector('.dir .caret svg')).not.toBeNull();
    // The same letters the files under it are marked with, rather than a pencil above an `A`.
    expect(dirs.some((d) => d?.includes('M 1') && d.includes('A 1'))).toBe(true);
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
    const onCommit = vi.fn();
    const { container, commit } = await panel({ onCommit });

    await fireEvent.input(container.querySelector('.summary') as HTMLInputElement, {
      target: { value: 'app: do the thing' },
    });
    await fireEvent.input(container.querySelector('.description') as HTMLTextAreaElement, {
      target: { value: 'Because the other thing was wrong.' },
    });
    await fireEvent.click(container.querySelector('.commit') as HTMLButtonElement);

    // Subject, blank line, body.
    expect(commit.message).toBe('app: do the thing\n\nBecause the other thing was wrong.');
    // And the panel asks the window to make it, rather than reaching for the engine itself:
    // the window's own commit is the same act, and a refusal has to be answered once.
    expect(onCommit).toHaveBeenCalledTimes(1);
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
        sections: new SectionsState(),
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onDiscard: () => {},
        onFileMenu: () => {},
        onCommit: () => {},
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
        sections: new SectionsState(),
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onDiscard: () => {},
        onFileMenu: () => {},
        onCommit: () => {},
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

describe('what a folder row says is under it', () => {
  /**
   * A closed directory that only says its name hides exactly the thing the panel exists to
   * show, so it carries a count per kind. A deletion used to fall into the edits: a folder
   * holding one modified file, one deleted and one added read "M 2  A 1" over rows marked
   * M, D and A, and the one change nobody wants to miss had no number of its own.
   */
  function tallies(container: HTMLElement): string[] {
    return [...container.querySelectorAll('.dir .count')].map((c) => c.textContent?.trim() ?? '');
  }

  it('counts a deletion as a deletion, not as an edit', () => {
    const worktree = new WorktreeState();
    worktree.status = {
      ...status(),
      entries: [
        entry('src/edited.txt'),
        entry('src/gone.txt', { worktree: 'deleted' }),
        entry('src/new.txt', { worktree: 'untracked' }),
      ],
    } as Status;
    const { container } = render(Staging, {
      props: {
        worktree,
        commit: new CommitState(),
        sections: new SectionsState(),
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onDiscard: () => {},
        onFileMenu: () => {},
        onCommit: () => {},
        onOpenFile: vi.fn(),
      },
    });
    expect(tallies(container)).toEqual(['M 1', 'A 1', 'D 1']);
  });

  it('leaves out the kinds that are not there', () => {
    const worktree = new WorktreeState();
    worktree.status = { ...status(), entries: [entry('src/edited.txt')] } as Status;
    const { container } = render(Staging, {
      props: {
        worktree,
        commit: new CommitState(),
        sections: new SectionsState(),
        branch: 'main',
        openPath: null,
        grouping: 'tree',
        onGrouping: () => {},
        onDiscard: () => {},
        onFileMenu: () => {},
        onCommit: () => {},
        onOpenFile: vi.fn(),
      },
    });
    expect(tallies(container)).toEqual(['M 1']);
  });
});

describe('the staging panel sections', () => {
  beforeEach(() => {
    localStorage.clear();
    invoke.mockReset();
    invoke.mockImplementation(async () => status());
  });

  function fold(container: HTMLElement, name: RegExp): HTMLButtonElement {
    const button = [...container.querySelectorAll<HTMLButtonElement>('button.fold')].find((b) =>
      name.test(b.textContent ?? ''),
    );
    if (!button) throw new Error(`no fold for ${name}`);
    return button;
  }

  const listDivider = '[role="separator"][aria-label="Resize the unstaged and staged lists"]';
  const composeDivider = '[role="separator"][aria-label="Resize the commit message"]';

  it('folds a list to its heading and takes its divider with it', async () => {
    const { container } = await panel();
    const unstaged = fold(container, /Unstaged files/u);
    expect(unstaged.getAttribute('aria-expanded')).toBe('true');
    expect(container.querySelector('#staging-unstaged')).not.toBeNull();
    expect(container.querySelector(listDivider)).not.toBeNull();

    await fireEvent.click(unstaged);
    expect(unstaged.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('#staging-unstaged')).toBeNull();
    // The heading and its count are still there, so the folded list says what is in it.
    expect(unstaged.textContent).toMatch(/Unstaged files \(3\)/u);
    expect(container.querySelector(listDivider)).toBeNull();
    expect(container.querySelector('#staging-staged')).not.toBeNull();

    await fireEvent.click(unstaged);
    expect(container.querySelector('#staging-unstaged')).not.toBeNull();
  });

  it('keeps the whole-list action on a folded list', async () => {
    // Staging everything does not need the list on screen, and folding it is often how a
    // long list of changes is got out of the way before doing exactly that.
    const { container, worktree } = await panel();
    await fireEvent.click(fold(container, /Unstaged files/u));
    const all = [...container.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Stage all changes',
    );
    expect(all).toBeDefined();
    const stage = vi.spyOn(worktree, 'stage').mockResolvedValue();
    if (all) await fireEvent.click(all);
    expect(stage).toHaveBeenCalledWith(
      ['src/app/App.svelte', 'src/state/graph.ts', 'docs/ARCHITECTURE.md'],
      true,
    );
  });

  it('shares the height between the lists as dragged', async () => {
    const sections = new SectionsState();
    sections.resize('unstagedShare', 0.3);
    const { container } = await panel({ sections });
    const [unstaged, staged] = container.querySelectorAll<HTMLElement>('.panel > section');
    expect(unstaged?.style.flexGrow).toBe('0.3');
    expect(Number(staged?.style.flexGrow)).toBeCloseTo(0.7);
  });

  it('gives the whole height to the list left open', async () => {
    const sections = new SectionsState();
    sections.resize('unstagedShare', 0.3);
    sections.toggle('staged');
    const { container } = await panel({ sections });
    const [unstaged, staged] = container.querySelectorAll<HTMLElement>('.panel > section');
    expect(unstaged?.style.flexGrow).toBe('1');
    expect(staged?.classList.contains('closed')).toBe(true);
    expect(staged?.style.flexGrow).toBe('');
  });

  it('sizes the message box from its divider, by key, and resets on a double-click', async () => {
    const sections = new SectionsState();
    const { container } = await panel({ sections });
    const divider = container.querySelector<HTMLElement>(composeDivider);
    expect(divider).not.toBeNull();
    const compose = container.querySelector<HTMLElement>('.compose');
    expect(compose?.style.height).toBe('');

    // Up makes the box taller: it is anchored to the foot of the panel.
    if (divider) await fireEvent.keyDown(divider, { key: 'ArrowUp' });
    expect(sections.sizes.composeHeight).toBe(SECTION_LIMITS.composeHeight.min);
    if (divider) await fireEvent.keyDown(divider, { key: 'ArrowUp', shiftKey: true });
    expect(sections.sizes.composeHeight).toBe(SECTION_LIMITS.composeHeight.min + 32);
    expect(compose?.style.height).toBe(`${SECTION_LIMITS.composeHeight.min + 32}px`);
    expect(compose?.classList.contains('sized')).toBe(true);
    if (divider) await fireEvent.keyDown(divider, { key: 'ArrowDown' });
    expect(sections.sizes.composeHeight).toBe(SECTION_LIMITS.composeHeight.min + 24);

    if (divider) await fireEvent.dblClick(divider);
    expect(sections.sizes.composeHeight).toBeNull();
    expect(compose?.style.height).toBe('');
  });

  it('commits from a folded message box', async () => {
    const onCommit = vi.fn();
    const { container, commit } = await panel({ onCommit });
    commit.summary = 'Fix the build';
    await fireEvent.click(fold(container, /Commit message/u));
    expect(container.querySelector('.summary')).toBeNull();
    expect(container.querySelector('.description')).toBeNull();
    expect(container.querySelector(composeDivider)).toBeNull();

    const button = container.querySelector<HTMLButtonElement>('button.commit');
    expect(button?.disabled).toBe(false);
    expect(button?.textContent).toMatch(/Commit 1 file/u);
    if (button) await fireEvent.click(button);
    expect(onCommit).toHaveBeenCalledOnce();
  });

  it('opens a folded message box when the caret is asked for', async () => {
    const sections = new SectionsState();
    sections.toggle('compose');
    const { container, commit } = await panel({ sections });
    expect(container.querySelector('.summary')).toBeNull();

    commit.focus();
    await waitFor(() => expect(document.activeElement?.classList.contains('summary')).toBe(true));
    expect(sections.collapsed.compose).toBe(false);
  });

  it('opens a folded message box when amending starts, and lets it be folded again', async () => {
    const sections = new SectionsState();
    sections.toggle('compose');
    const { container, commit } = await panel({ sections });
    commit.amend = true;
    await waitFor(() => expect(container.querySelector('.summary')).not.toBeNull());

    // Folding it again while still amending is the user's choice, and is not undone.
    await fireEvent.click(fold(container, /Commit message/u));
    await tick();
    expect(sections.collapsed.compose).toBe(true);
    expect(container.querySelector('.summary')).toBeNull();
  });

  it('opens the list that holds a file put on screen from elsewhere', async () => {
    const sections = new SectionsState();
    sections.toggle('unstaged');
    const { rerender } = await panel({ sections, openPath: null });
    await rerender({ openPath: 'docs/ARCHITECTURE.md' });
    expect(sections.collapsed.unstaged).toBe(false);
  });

  it('leaves a fold alone when the panel comes back on the same file', async () => {
    const sections = new SectionsState();
    sections.toggle('unstaged');
    await panel({ sections, openPath: 'docs/ARCHITECTURE.md' });
    await tick();
    expect(sections.collapsed.unstaged).toBe(true);
  });

  it('fills the panel with the message box when both lists are folded', async () => {
    const sections = new SectionsState();
    sections.toggle('unstaged');
    sections.toggle('staged');
    const { container } = await panel({ sections });
    expect(container.querySelector('.compose')?.classList.contains('fill')).toBe(true);
    // Nothing to share the height with, so there is nothing to drag.
    expect(container.querySelector(composeDivider)).toBeNull();
    expect(container.querySelector(listDivider)).toBeNull();
  });

  it('remembers a fold for the next window', async () => {
    const { container } = await panel();
    await fireEvent.click(fold(container, /Staged files/u));
    expect(new SectionsState().collapsed.staged).toBe(true);
  });

  it('shortens a heading in a narrow panel before moving the buttons beside it', () => {
    const css = readFileSync(resolve(process.cwd(), 'src/app/Staging.svelte'), 'utf8');
    expect(css).toMatch(/h3 \.fold \.title \{[^}]*text-overflow:\s*ellipsis/u);
    for (const action of ['all', 'expand']) {
      expect(css).toMatch(new RegExp(`\\.${action} \\{[^}]*flex: 0 0 auto; white-space: nowrap`, 'u'));
    }
  });
});

describe('drafting a message', () => {
  it('offers Draft only when drafting is on, and only with something staged', async () => {
    const asked = vi.fn();
    const { container } = await panel({ onDraft: asked });
    const button = container.querySelector('button.draft') as HTMLButtonElement;
    expect(button.textContent).toBe('Draft');
    expect(button.disabled).toBe(false);
    await fireEvent.click(button);
    expect(asked).toHaveBeenCalledOnce();

    const off = await panel();
    expect(off.container.querySelector('button.draft')).toBeNull();
  });

  it('says it is drafting while the answer is on its way', async () => {
    const { container } = await panel({ onDraft: vi.fn(), drafting: true });
    const button = container.querySelector('button.draft') as HTMLButtonElement;
    expect(button.textContent).toBe('Drafting…');
    expect(button.disabled).toBe(true);
  });
});

describe('in another language', () => {
  it('names the lists and the commit button in German, with the plural German uses', async () => {
    const { locale } = await import('../../src/state/locale.svelte');
    locale.language = 'de';
    try {
      const { container, commit } = await panel();
      expect(container.textContent).toContain('Vorgemerkt');
      // The count above the lists is German too, not an English "file change" beside "auf".
      expect(container.querySelector('header .count')?.textContent).toMatch(/\d+ Änderung/u);
      commit.summary = 'ui: etwas';
      await tick();
      const button = container.querySelector('button.commit') as HTMLButtonElement;
      expect(button.textContent?.trim()).toBe('1 Datei committen');
    } finally {
      locale.language = 'en';
    }
  });
});
