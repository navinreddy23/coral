// @vitest-environment happy-dom
import { cleanup, render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * The shell driven through a real encoded frame.
 *
 * The graph rows are decoded by the same reader the window uses, from the same golden fixture
 * the Rust encoder is checked against, so a row's date and hash come out of the typed arrays
 * exactly as they do in the app. That is the part worth testing here: the rows are indexed
 * into a frame that is a window into the graph, and getting that wrong shows another commit's
 * details under the right summary.
 */
// Resolved from the working directory: under a DOM environment `import.meta.url` is an http
// URL, which node:fs will not open.
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
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
// Handlers are kept so a test can deliver a repository change, which is the only way to
// exercise what the window does when refs move underneath it.
const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler);
    return () => listeners.delete(name);
  },
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
import { open as openDialog } from '@tauri-apps/plugin-dialog';

import App from '../../src/app/App.svelte';

// Each test mounts its own window. Without this the previous one is still in the document, and
// a query for something every window has finds one per window.
afterEach(cleanup);
import { decodeFrame, oidOf } from '../../src/graph/frame';

/** The same rows the window decodes, to compare what it put on screen against. */
const frame = decodeFrame(
  frameBytes.buffer.slice(frameBytes.byteOffset, frameBytes.byteOffset + frameBytes.byteLength),
);

const REPO = '/repo';

function answers(over: Record<string, unknown> = {}): Record<string, unknown> {
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
    // The same pattern the engine sends, so the transport check passes rather than putting a
    // warning banner up in place of the graph.
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
    graph_scope: { solo: null, hidden: [] },
    set_graph_scope: { solo: null, hidden: [] },
    graph_rewalk: null,
    repo_submodules: [],
    patch_range_size: 3,
    compare_commits: [],
    commit_detail: null,
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
    session_get: { tabs: [{ id: 1, path: REPO, group: null }], active: 1, groups: [] },
    tab_open: { tabs: [{ id: 1, path: REPO, group: null }], active: 1, groups: [] },
    tab_activate: { tabs: [{ id: 1, path: REPO, group: null }], active: 1, groups: [] },
    ...over,
  };
}

/** Enough of an ssh answer for the pane to render. */
function sshScopes() {
  const blank = { useAgent: true, privateKey: '', publicKey: '', command: '', credentialHelper: '' };
  return { effective: blank, global: blank, local: {} };
}

/** The same for signing. */
function signingScopes() {
  const blank = { format: 'openpgp', program: '', key: '', signCommits: false, signTags: false };
  return { effective: blank, global: blank, local: {} };
}

function wire(over: Record<string, unknown> = {}) {
  const table = answers(over);
  invoke.mockImplementation(async (cmd: string) => {
    if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
    // An answer that is an error is one the engine refuses to give, which is a case the window
    // has to survive as much as any other.
    const answer = table[cmd];
    if (answer instanceof Error) throw answer;
    return answer;
  });
}

async function shell(over: Record<string, unknown> = {}) {
  wire(over);
  const view = render(App);
  await waitFor(() => {
    if (view.container.querySelectorAll('li.row').length === 0) throw new Error('no rows yet');
  });
  return view;
}

describe('the shell', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  it('renders a row per visible commit', async () => {
    // The count follows the viewport, which a DOM without layout reports as zero; what has to
    // hold is that rows appear at all and each is a real commit.
    const { container } = await shell();
    const rows = container.querySelectorAll('li.row');
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThanOrEqual(frame.rowCount);
  });

  it('shows each row the hash of its own commit', async () => {
    // Rows are absolute and the frame is a window into them, so an off-by-one here puts one
    // commit's date and hash under another's summary.
    const { container } = await shell();
    const shown = [...container.querySelectorAll('li.row .sha')].map((e) => e.textContent);
    const expected = shown.map((_, row) => oidOf(frame, row).slice(0, 8));
    expect(shown).toEqual(expected);
    expect(new Set(shown).size).toBe(shown.length);
  });

  it('tints the message alone when a row is selected, not the whole row', async () => {
    // A full-width tint paints over the lanes and the ref pills, which are the two things you
    // are looking at when you pick a commit.
    const { container } = await shell();
    const row = container.querySelectorAll('li.row')[2] as HTMLElement;
    await fireEvent.click(row.querySelector('button.hit') as HTMLButtonElement);
    await waitFor(() => {
      if (!row.classList.contains('selected')) throw new Error('not selected yet');
    });
    // Svelte scopes every selector, so the rules are matched by shape rather than by text.
    // What matters is the paint: a rule may style the text of a selected row freely, but
    // anything that fills a background has to reach the message cell to do it, or the tint
    // covers the lanes and the ref pills too.
    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((rule) => rule.cssText)
      .filter((text) => text.includes('row.selected'));
    expect(rules.length).toBeGreaterThan(0);

    const painting = rules.filter((rule) => /background(-color)?:/u.test(rule));
    expect(painting.length, 'something must paint the selection').toBeGreaterThan(0);
    // The strip counts as reaching it: it is a child of the message cell, drawn in the gap
    // between the node and the text, so it is inside the same column and covers neither the
    // lanes nor the pills.
    for (const rule of painting) {
      expect(rule, rule).toMatch(/cell\.message|lane-strip/u);
    }
  });

  it('puts the lane colour in the strip beside the message, not across it', async () => {
    // A message cell washed with its lane colour is what made the selected commit hard to
    // find: every row was tinted something, so the one tint that means "you picked this" was
    // just another colour in the column.
    const { container } = await shell();
    const row = container.querySelectorAll('li.row')[1] as HTMLElement;
    const cell = row.querySelector('.cell.message') as HTMLElement;
    const strip = cell.querySelector('.lane-strip');
    expect(strip, 'every commit row carries a lane strip').not.toBeNull();

    // The lane colour is named on the cell as a custom property and spent by the strip alone.
    expect(cell.style.getPropertyValue('--row-tint')).toMatch(/lane-\d+-soft/u);
    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((rule) => rule.cssText);
    const cellPaint = rules.filter(
      (text) => /\.row[^{]*\.cell\.message[^{]*\{/u.test(text) && /--row-tint/u.test(text),
    );
    expect(cellPaint, 'the message cell must not wear the lane colour').toEqual([]);
    expect(
      rules.some((text) => /lane-strip/u.test(text) && /--row-tint/u.test(text)),
      'the strip must be what wears it',
    ).toBe(true);
  });

  it('commits from the keyboard, and keeps the message when the panel is not showing', async () => {
    // The message lives in the window, not the panel: selecting a commit unmounts the panel,
    // and a half-written message used to go with it.
    const { container } = await shell({
      repo_status: {
        entries: [{ path: 'a.txt', staged: 'modified', worktree: null, conflict: null }],
        conflicted: [],
      },
      commit_staged: { entries: [], conflicted: [] },
    });

    // The panel opens with the working copy, which is a row of its own at the top of the list.
    await waitFor(() => {
      if (!container.querySelector('button.row.wip')) throw new Error('no working copy row yet');
    });
    await fireEvent.click(container.querySelector('button.row.wip') as HTMLButtonElement);
    await waitFor(() => {
      if (!container.querySelector('input.summary')) throw new Error('no staging panel yet');
    });
    const summary = container.querySelector('input.summary') as HTMLInputElement;
    summary.value = 'a message typed by hand';
    await fireEvent.input(summary);

    invoke.mockClear();
    // On the field, because that is where the key is pressed: the binding is scoped to the
    // message box rather than the whole window, so Enter elsewhere means Enter.
    await fireEvent.keyDown(summary, { key: 'Enter', ctrlKey: true });
    await waitFor(() => {
      const committed = invoke.mock.calls.find((c) => c[0] === 'commit_staged');
      if (!committed) throw new Error('no commit yet');
      expect((committed[1] as { message?: string }).message).toBe('a message typed by hand');
    });
  });

  it('makes the graph labels draggable, except the ones nothing can be done with', async () => {
    const { container } = await shell({
      repo_refs: [
        {
          name: 'refs/heads/spike',
          short: 'spike',
          kind: { kind: 'local_branch' },
          target: frameOids[1],
          peeled: null,
          upstream: null,
          ahead: 0,
          behind: 0,
          row: 1,
        },
        {
          name: 'refs/tags/v1',
          short: 'v1',
          kind: { kind: 'tag', annotated: false },
          target: frameOids[2],
          peeled: null,
          upstream: null,
          ahead: 0,
          behind: 0,
          row: 2,
        },
      ],
    });

    await waitFor(() => {
      const names = [...container.querySelectorAll('.pill-text')].map((n) => n.textContent);
      if (!names.includes('spike')) throw new Error('no pills yet');
    });
    const pills = [...container.querySelectorAll('button.pill')] as HTMLElement[];
    const branch = pills.find((p) => p.textContent?.includes('spike'));
    const tag = pills.find((p) => p.textContent?.includes('v1'));

    // A branch can be dropped onto another to merge, rebase or push. A tag cannot: there is
    // nothing the gesture would mean.
    expect(branch?.getAttribute('draggable')).toBe('true');
    expect(tag?.getAttribute('draggable')).toBe('false');
  });

  it('fetches from the keyboard', async () => {
    await shell({ repo_action: { what: 'fetch', message: '', conflicted: false } });
    invoke.mockClear();
    await fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
    await waitFor(() => {
      const fetched = invoke.mock.calls.find(
        (c) =>
          c[0] === 'repo_action' &&
          (c[1] as { action?: { kind?: string } })?.action?.kind === 'fetch',
      );
      if (!fetched) throw new Error('no fetch yet');
    });
  });

  it('hides and restores the toolbar from the keyboard', async () => {
    const { container } = await shell();
    const toolbar = () => container.querySelector('.toolbar');
    expect(toolbar(), 'the toolbar starts visible').not.toBeNull();

    await fireEvent.keyDown(window, { key: 'u', ctrlKey: true });
    await waitFor(() => {
      if (toolbar() !== null) throw new Error('still there');
    });
    await fireEvent.keyDown(window, { key: 'u', ctrlKey: true });
    await waitFor(() => {
      if (toolbar() === null) throw new Error('did not come back');
    });
  });

  it('puts the caret in the branch filter when / is pressed', async () => {
    const { container } = await shell();
    const filter = container.querySelector('input.filter') as HTMLInputElement;
    expect(filter, 'the panel has a filter box').not.toBeNull();
    expect(document.activeElement).not.toBe(filter);

    await fireEvent.keyDown(window, { key: '/' });
    await waitFor(() => {
      if (document.activeElement !== filter) throw new Error('not focused yet');
    });
  });

  it('offers to apply a patch when nothing is being compared', async () => {
    const { container } = await shell();
    const dialog = vi.mocked(openDialog);
    dialog.mockClear();
    dialog.mockResolvedValue(null);

    const patch = [...container.querySelectorAll('button.action')].find(
        (b) => b.getAttribute('aria-label') === 'Patch',
      ) as HTMLButtonElement;
    expect(patch, 'the toolbar carries a patch button').toBeDefined();
    await fireEvent.click(patch);

    await waitFor(() => {
      if (dialog.mock.calls.length === 0) throw new Error('no dialog yet');
    });
    // Several files, because a series is several files, and not a directory.
    expect(dialog.mock.calls[0]?.[0]).toMatchObject({ multiple: true });
  });

  it('offers to write patches out when two commits are being compared', async () => {
    const { container } = await shell();
    const rows = [...container.querySelectorAll('li.row')] as HTMLElement[];
    await fireEvent.click(rows[2]?.querySelector('button.hit') as HTMLButtonElement);
    await fireEvent.click(rows[0]?.querySelector('button.hit') as HTMLButtonElement, {
      ctrlKey: true,
    });
    // The button says which of its two jobs it will do, so that is what says the pair took.
    const patchButton = () =>
      [...container.querySelectorAll('button.action')].find(
        (b) => b.getAttribute('aria-label') === 'Patch',
      ) as HTMLButtonElement;
    await waitFor(() => {
      if (!patchButton().title.includes('write the commits')) {
        throw new Error('not comparing yet');
      }
    });

    const dialog = vi.mocked(openDialog);
    dialog.mockClear();
    dialog.mockResolvedValue(null);
    invoke.mockClear();
    await fireEvent.click(patchButton());

    // The size of the range is asked first: two commits picked far apart is a file per commit
    // between them, and on a large repository that is a great many files.
    await waitFor(() => {
      const asked = invoke.mock.calls.some((c) => c[0] === 'patch_range_size');
      if (!asked) throw new Error('did not ask how many');
    });
    await waitFor(() => {
      if (dialog.mock.calls.length === 0) throw new Error('no dialog yet');
    });
    expect(dialog.mock.calls[0]?.[0]).toMatchObject({ directory: true });
  });

  it('runs undo and redo from the keyboard, not only from the toolbar', async () => {
    // Both were listed in the shortcut help and rebindable, and both did nothing: the key
    // dispatch had no case for them. For Ctrl+Z of all keys that is worse than not offering it.
    await shell({ repo_action: { what: 'undo', message: '', conflicted: false } });
    invoke.mockClear();

    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    await waitFor(() => {
      const undo = invoke.mock.calls.find(
        (c) => c[0] === 'repo_action' && (c[1] as { action?: { kind?: string } })?.action?.kind === 'undo',
      );
      if (!undo) throw new Error('no undo yet');
    });

    invoke.mockClear();
    await fireEvent.keyDown(window, { key: 'y', ctrlKey: true });
    await waitFor(() => {
      const redo = invoke.mock.calls.find(
        (c) => c[0] === 'repo_action' && (c[1] as { action?: { kind?: string } })?.action?.kind === 'redo',
      );
      if (!redo) throw new Error('no redo yet');
    });
  });

  it('gives the whole pane to the merge tool while an operation is stopped', async () => {
    // The panel beside it can only offer a commit to select, so it spent half the window
    // saying so while the two sides being merged were squeezed into a column too narrow to
    // read, with the button that takes a side clipped off the end of it.
    const { container } = await shell({
      repo_operation: {
        state: 'merge',
        labels: { ours: 'main', theirs: 'side', swapped: false },
        progress: null,
        headName: 'main',
        stoppedAt: null,
        interactive: false,
        resumable: true,
      },
      repo_conflicts: [{ path: 'shared.txt', kind: 'both_modified' }],
    });

    await waitFor(() => {
      if (!container.textContent?.includes('merge in progress')) throw new Error('not stopped yet');
    });
    expect(container.textContent).not.toContain('Select a commit');
    expect(container.querySelector('aside.wip-panel')).toBeNull();
  });

  it('shows the working copy at once in a repository with nothing committed', async () => {
    // The graph says "the panel on the right makes the first commit", and that was the first
    // sentence a stranger read — pointing at a panel showing "a commit's author, message and
    // files appear here". There is no commit to select, so there is nothing else it could show.
    // A frame with no rows in it: the same golden header, with the two counts set to zero.
    const empty = frameBytes.buffer.slice(
      frameBytes.byteOffset,
      frameBytes.byteOffset + frameBytes.byteLength,
    );
    const header = new DataView(empty);
    header.setUint32(12, 0, true);
    header.setUint32(16, 0, true);

    wire({
      graph_frame: empty,
      repo_status: {
        entries: [{ path: 'README.md', staged: null, worktree: 'untracked', conflict: null }],
        conflicted: [],
      },
    });
    const view = render(App);
    const { container } = view;

    await waitFor(() => {
      if (!container.querySelector('aside.wip-panel')) throw new Error('not showing it yet');
    });
    expect(container.textContent).toContain('makes the first commit');
    expect(container.textContent, 'and not the panel that waits for one').not.toContain(
      "A commit's author",
    );
  });

  it('asks who you are when git refuses the first commit for want of a name', async () => {
    /*
     * On a machine where git has never been told a name it refuses and explains itself in
     * nine lines about `git config --global`, which is a terminal's answer to a question
     * asked in a window. This is the first thing a new user does, so it is asked here.
     */
    const { container } = await shell({
      repo_status: {
        entries: [{ path: 'README.md', staged: 'added', worktree: null, conflict: null }],
        conflicted: [],
      },
      commit_staged: new Error(
        "git commit exited with 128: fatal: unable to auto-detect email address (got 'x@y.(none)')",
      ),
    });

    await waitFor(() => {
      if (!container.querySelector('button.row.wip')) throw new Error('no working copy row yet');
    });
    await fireEvent.click(container.querySelector('button.row.wip') as HTMLButtonElement);
    await waitFor(() => {
      if (!container.querySelector('input.summary')) throw new Error('no staging panel yet');
    });
    const summary = container.querySelector('input.summary') as HTMLInputElement;
    summary.value = 'Start the notebook';
    await fireEvent.input(summary);
    // The button, not the keystroke: the panel used to commit by a path of its own, so a fix
    // in the window only ever ran down one of the two.
    await fireEvent.click(container.querySelector('button.commit') as HTMLButtonElement);

    const dialog = await waitFor(() => {
      const found = container.querySelector('[role="dialog"]');
      if (!found) throw new Error('nothing asked');
      return found as HTMLElement;
    });
    expect(dialog.textContent).toContain('Git does not know who you are');
    // Not git's own advice, which is what was shown before.
    expect(dialog.textContent).not.toContain('git config --global');
  });

  it('takes down the stopped notice once nothing is stopped', async () => {
    /*
     * "merge stopped on conflicts" waits to be dismissed, which is right for a failure nobody
     * was watching and wrong once the window itself has settled it: the notice sat in the
     * corner through resolving that merge, committing it, and everything after.
     */
    const stopped = {
      state: 'merge',
      labels: { ours: 'main', theirs: 'side', swapped: false },
      progress: null,
      headName: 'main',
      stoppedAt: null,
      interactive: false,
      resumable: true,
    };
    const { container } = await shell({
      repo_action: { what: 'merge side', conflicted: true, message: 'CONFLICT (content)' },
      repo_operation: null,
      repo_conflicts: [],
    });

    // Merge something, and have it stop.
    wire({
      repo_action: { what: 'merge side', conflicted: true, message: 'CONFLICT (content)' },
      repo_operation: stopped,
      repo_conflicts: [{ path: 'shared.txt', kind: 'both_modified' }],
    });
    // Any action will do: what makes the notice is the outcome saying it stopped.
    await fireEvent.click(container.querySelector('[aria-label="Fetch"]') as HTMLButtonElement);

    const toast = await waitFor(() => {
      const found = [...container.querySelectorAll('.toast')].find((t) =>
        t.textContent?.includes('stopped on conflicts'),
      );
      if (!found) throw new Error('no notice yet');
      return found;
    });
    expect(toast).toBeTruthy();

    // Now settle it: the operation is over.
    wire({ repo_operation: null, repo_conflicts: [] });
    await fireEvent.keyDown(window, { key: 'r', ctrlKey: true });
    await waitFor(() => {
      const still = [...container.querySelectorAll('.toast')].some((t) =>
        t.textContent?.includes('stopped on conflicts'),
      );
      if (still) throw new Error('the notice is still up');
    });
  });

  it('cycles the left panel from open, to a rail, to gone, and back', async () => {
    // The owner asked for hide or minimise. Three states behind one button, so what matters is
    // that a press always changes something and three of them come back to the start.
    const { container } = await shell();
    const press = async () => {
      const button = container.querySelector('[aria-label="Left panel"]') as HTMLButtonElement;
      await fireEvent.click(button);
    };
    const showing = () => ({
      panel: container.querySelector('aside .viewing') !== null,
      rail: container.querySelector('nav.rail') !== null,
    });

    expect(showing(), 'the window opens with the panel out').toEqual({ panel: true, rail: false });
    await press();
    expect(showing(), 'minimised to the rail').toEqual({ panel: false, rail: true });
    await press();
    expect(showing(), 'gone').toEqual({ panel: false, rail: false });
    await press();
    expect(showing(), 'and back').toEqual({ panel: true, rail: false });
  });

  it('opens the panel at the section a rail icon stands for', async () => {
    // A rail icon that opened the panel onto a collapsed heading would be a click that
    // appeared not to work. Tags ship collapsed, so this is the one that shows it.
    const { container } = await shell();
    await fireEvent.click(container.querySelector('[aria-label="Left panel"]') as HTMLButtonElement);
    await waitFor(() => {
      if (!container.querySelector('nav.rail')) throw new Error('no rail yet');
    });

    await fireEvent.click(container.querySelector('nav.rail [aria-label="Tags"]') as HTMLElement);
    await waitFor(() => {
      if (!container.querySelector('aside')) throw new Error('the panel is not back');
    });
    const tags = container.querySelector('section[data-section="tags"]') as HTMLElement;
    expect(tags, 'the section is in the panel').not.toBeNull();
    expect(tags.querySelector('ul'), 'and open, not still collapsed').not.toBeNull();
  });

  it('sticks the working copy row to the top of the list rather than below it', async () => {
    // Sticky moves the element and nothing else, so an offset here leaves the first commit
    // where it was and drops the working copy row on top of it. It was held 22px clear for a
    // column header the redesign removed, and 22 of the first commit's 28 pixels went under
    // it — its ref pills, its node and the ring on it. happy-dom does not lay out, so this
    // reads the rule.
    await shell();
    const rule = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText)
      .find((text) => /\.wip[^{]*\{/u.test(text) && /position:\s*sticky/u.test(text));
    expect(rule, 'the working copy row is pinned').toBeDefined();
    expect(rule, 'and pinned at the very top').toMatch(/top:\s*0(px)?\s*;/u);
  });

  it('keeps the row click target above the cells that are positioned', async () => {
    // The message cell is a positioned element, because the lane band hangs off it, and it
    // comes after the click overlay in the row. Without a raise on the overlay the cell sat on
    // top of it and clicking a commit's message did nothing at all — only the avatar, drawn on
    // the canvas, still selected the row. happy-dom does not stack, so this reads the rule.
    await shell();
    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText);

    const hit = rules.find((text) => /\.hit[^{]*\{/u.test(text) && /position:\s*absolute/u.test(text));
    expect(hit, 'the overlay rule').toBeDefined();
    expect(hit, 'the overlay has to be raised above the positioned cells').toMatch(/z-index/u);

    // And the pills stay above the overlay in turn, or a branch name loses its own click.
    const refs = rules.find((text) => /\.cell\.refs[^{]*\{/u.test(text) && /z-index/u.test(text));
    expect(refs, 'the ref column keeps its raise').toBeDefined();
  });

  it('sizes the lane band to the empty part of the lane column', async () => {
    // The band fills the corridor between a row's outermost lane and its message, so its width
    // is per-row and comes from the geometry. It has been silently zero once already: a flex
    // basis of zero beat the width and the colour disappeared with nothing to say so.
    const { container } = await shell();
    const strips = [...container.querySelectorAll('li.row .lane-strip')] as HTMLElement[];
    expect(strips.length, 'every commit row carries one').toBeGreaterThan(0);
    for (const strip of strips) {
      expect(strip.style.getPropertyValue('--lane-gap')).toMatch(/^\d+(\.\d+)?px$/u);
    }

    const rule = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText)
      .find((text) => /\.lane-strip[^{]*\{/u.test(text) && /--lane-gap/u.test(text));
    expect(rule, 'the band is sized from that property').toBeDefined();
    // Out of flow and anchored to the cell's leading edge, or it paints over the padding where
    // the selected row's bar is drawn.
    expect(rule).toMatch(/position:\s*absolute/u);
    expect(rule).toMatch(/right:\s*100%/u);
    // A flex basis would beat the width. There must not be one.
    expect(rule).not.toMatch(/flex:/u);
  });

  /** One local branch sitting on a row the fixture frame actually holds. */
  function aBranchOnRow(row: number) {
    return [
      {
        name: 'refs/heads/spike',
        short: 'spike',
        kind: { kind: 'local_branch' },
        target: frameOids[row],
        peeled: null,
        upstream: null,
        ahead: 0,
        behind: 0,
        row,
      },
    ];
  }

  it('draws a branch pill for a ref that is shown', async () => {
    const { container } = await shell({ repo_refs: aBranchOnRow(1) });
    await waitFor(() => {
      const names = [...container.querySelectorAll('.pill-text')].map((n) => n.textContent);
      if (!names.includes('spike')) throw new Error('no pill yet');
    });
  });

  it('draws no pill for a ref that is hidden', async () => {
    // Hiding takes a branch out of the walk, but its commits usually stay: another branch
    // still reaches them. The label must not stay with them, or the graph goes on naming a
    // branch the panel beside it is showing as hidden.
    const { container } = await shell({
      repo_refs: aBranchOnRow(1),
      graph_scope: { solo: null, hidden: ['refs/heads/spike'] },
    });
    await waitFor(() => {
      if (container.querySelectorAll('li.row').length === 0) throw new Error('no rows yet');
    });
    const names = [...container.querySelectorAll('.pill-text')].map((n) => n.textContent);
    expect(names).not.toContain('spike');
  });

  it('keeps drawing the other labels while one branch is soloed', async () => {
    // Solo restricts the walk, not the labelling: a tag on a commit the soloed branch reaches
    // is still worth drawing, and only a ref hidden by hand loses its pill.
    const { container } = await shell({
      repo_refs: aBranchOnRow(1),
      graph_scope: { solo: 'refs/heads/other', hidden: [] },
    });
    await waitFor(() => {
      const names = [...container.querySelectorAll('.pill-text')].map((n) => n.textContent);
      if (!names.includes('spike')) throw new Error('no pill yet');
    });
  });

  it('rereads the scope before rewalking when refs move, so a deleted solo cannot strand the graph', async () => {
    // Deleting the soloed branch in a terminal used to leave an empty graph under a banner
    // naming it: the walk was redone against a name the repository no longer had, and nothing
    // asked the engine to prune it. Reading the scope is what prunes, so it has to come first.
    await shell({ graph_scope: { solo: 'refs/heads/gone', hidden: [] } });
    await waitFor(() => {
      if (!listeners.has('repo://changed')) throw new Error('not listening yet');
    });
    invoke.mockClear();

    const deliver = listeners.get('repo://changed');
    expect(deliver).toBeDefined();
    deliver?.({
      payload: { refs: true, index: false, worktree: false, ops: false, graph: false },
    });

    await waitFor(() => {
      const called = invoke.mock.calls.map((c) => c[0] as string);
      if (!called.includes('graph_rewalk')) throw new Error('no rewalk yet');
    });
    const called = invoke.mock.calls.map((c) => c[0] as string);
    expect(called).toContain('graph_scope');
    expect(called.indexOf('graph_scope')).toBeLessThan(called.indexOf('graph_rewalk'));
  });

  it('keeps the lanes in their own column, clear of the branch names', async () => {
    const { container } = await shell();
    const row = container.querySelector('li.row') as HTMLElement;
    const cells = [...row.querySelectorAll('.cell')].map((c) => c.className);
    // Refs, then the lane column, then the message: the canvas is positioned over the middle
    // one, so a missing or reordered cell is what lets it cover the pills.
    expect(cells[0]).toContain('refs');
    expect(cells[1]).toContain('graph-col');
    expect(cells[2]).toContain('message');
  });

  /**
   * The start page is what a new tab is, and the tab strip presents it as one: a selected tab
   * beside the repository's. The status line under it went on naming the other tab's branch
   * and counting its commits, over a page with no repository on it at all.
   */
  it('says nothing about a repository while the new-tab page is up', async () => {
    const { container } = await shell();
    await waitFor(() => {
      if (!container.querySelector('footer.status')) throw new Error('no status bar yet');
    });

    await fireEvent.keyDown(window, { key: 't', ctrlKey: true });
    await waitFor(() => {
      if (!container.querySelector('.start')) throw new Error('no start page');
    });
    expect(container.querySelector('footer.status')).toBeNull();
  });

  it('opens the palette on its shortcut and closes it again', async () => {
    const { container } = await shell();
    await fireEvent.keyDown(window, { key: 'p', ctrlKey: true });
    await waitFor(() => {
      if (!container.querySelector('.panel input')) throw new Error('no palette');
    });
    await fireEvent.keyDown(container.querySelector('.panel input') as HTMLInputElement, {
      key: 'Escape',
    });
    await waitFor(() => {
      if (container.querySelector('.panel input')) throw new Error('palette still open');
    });
  });

  /**
   * The list already leaves out a theme entry that would change nothing. These two were the
   * exception: "Pop the latest stash" with nothing stashed answered "stash@{0} is not a valid
   * reference", and "Stash changes" with a clean working copy did nothing at all.
   */
  it('leaves out the stash commands that have nothing to act on', async () => {
    const { container } = await shell();
    await fireEvent.keyDown(window, { key: 'p', ctrlKey: true });
    const input = await waitFor(() => {
      const found = container.querySelector('.panel input') as HTMLInputElement | null;
      if (!found) throw new Error('no palette');
      return found;
    });
    await fireEvent.input(input, { target: { value: 'stash' } });
    const offered = [...container.querySelectorAll('.panel li')].map((r) => r.textContent ?? '');
    expect(offered.some((t) => t.includes('Pop the latest stash'))).toBe(false);
    expect(offered.some((t) => t.includes('Stash changes'))).toBe(false);
  });

  it('turns a pull it cannot fast-forward into the choice it actually is', async () => {
    // The button pulls fast-forward only, which is the safe reading of "bring me up to date".
    // When both sides have moved git refuses, and refusing is right — but it said so by
    // dumping four lines of advice about `git config pull.rebase` into a toast.
    const { container } = await shell({
      repo_action: new Error(
        "git pull exited with 128: hint: Diverging branches can't be fast-forwarded, you need " +
          'to either:\nhint:\nhint:  git merge --no-ff',
      ),
    });

    const pull = container.querySelector('[aria-label="Pull"]') as HTMLButtonElement;
    await fireEvent.click(pull);

    const dialog = await waitFor(() => {
      const found = container.querySelector('[role="dialog"]');
      if (!found) throw new Error('no question yet');
      return found as HTMLElement;
    });
    expect(dialog.textContent, 'and it says which two things they are').toContain('Pull, merging');
    expect(dialog.textContent).toContain('Pull, rebasing');
    // Not the raw advice, which is what was there before.
    expect(dialog.textContent).not.toContain('pull.rebase');
  });

  it('offers to send the tags with a push, since git sends none by itself', async () => {
    const { container } = await shell({
      repo_action: { what: 'push', conflicted: false, message: '' },
      repo_refs: [
        {
          name: 'refs/tags/v1.0',
          short: 'v1.0',
          kind: { kind: 'tag', annotated: false },
          target: 'a'.repeat(40),
          peeled: null,
          upstream: null,
          ahead: 0,
          behind: 0,
          row: 0,
        },
      ],
    });

    // The refs are a second read; a menu built before they land knows of no tags and offers
    // the item disabled.
    await waitFor(() => {
      if (!container.querySelector('.pill-text')) throw new Error('no refs yet');
    });

    const caret = [...container.querySelectorAll('button.caret')].find(
      (b) => b.getAttribute('title') === 'Choose what to push',
    ) as HTMLButtonElement;
    expect(caret, 'the push button has a caret of its own').toBeTruthy();
    await fireEvent.click(caret);

    const labels = await waitFor(() => {
      const found = [...container.querySelectorAll('.menu .label')].map((e) => e.textContent?.trim());
      if (found.length === 0) throw new Error('no menu');
      return found;
    });
    expect(labels).toEqual([
      'Push this branch',
      'Force push this branch',
      'Push this branch and every tag',
    ]);

    const withTags = [...container.querySelectorAll('.menu .label')].find(
      (e) => e.textContent?.trim() === 'Push this branch and every tag',
    );
    await fireEvent.click(withTags?.closest('button') as HTMLButtonElement);

    await waitFor(() => {
      const call = invoke.mock.calls.filter(([cmd]) => cmd === 'repo_action').at(-1);
      expect((call?.[1] as { action: unknown }).action).toEqual({
        kind: 'push',
        remote: null,
        setUpstream: true,
        refspec: null,
        tags: true,
        forceWithLease: false,
        delete: false,
      });
    });
  });

  it('closes the search bar on Escape, wherever the focus went', async () => {
    // Escape was bound to the search field alone, so clicking a result — which is the whole
    // point of the bar — left no way to dismiss it but finding the small button.
    const { container } = await shell();
    await fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    await waitFor(() => {
      if (!container.querySelector('.find input')) throw new Error('no search bar');
    });

    (container.querySelector('li.row button.hit') as HTMLButtonElement | null)?.click();
    await fireEvent.keyDown(window, { key: 'Escape' });
    await waitFor(() => {
      if (container.querySelector('.find input')) throw new Error('the bar is still up');
    });
  });

  it('asks about the host when a repository opens', async () => {
    // Not awaited during open — a slow or unreachable host must not hold up the window — so
    // the only thing that says it happened is that the call was made at all.
    await shell();
    await waitFor(() => {
      const asked = invoke.mock.calls.some(([cmd]) => cmd === 'hosting_status');
      if (!asked) throw new Error('never asked about the host');
    });
  });

  it('shows the merge tool instead of the graph when a merge has stopped', async () => {
    const { container } = await shell({
      repo_operation: {
        state: 'merge',
        labels: { ours: 'master', theirs: 'side', swapped: false },
        progress: null,
        headName: null,
        stoppedAt: null,
        interactive: false,
        resumable: true,
      },
      repo_conflicts: [
        { path: 'f.txt', kind: 'both_modified', binary: false, deleteModify: false },
      ],
    });
    await waitFor(() => {
      if (!container.querySelector('section.merge')) throw new Error('no merge tool');
    });
    // The list and the two handles that size its columns go together, so what is hidden is
    // the wrapper around both rather than the scroller alone.
    expect((container.querySelector('.plot') as HTMLElement).className).toContain('hidden');
    expect(container.querySelector('.plot .graph'), 'still mounted, so it keeps its scroll')
      .not.toBeNull();
  });
});

describe('a git too old to open anything', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  /** What the engine answers when the machine's git is below the floor. */
  function tooOld() {
    return {
      open_repo: Object.assign(new Error('git 2.20.1 is too old; coral needs 2.40.0 or newer'), {
        code: 'git_too_old',
      }),
      experimental_git: {
        chosen: { kind: 'system' },
        candidates: [
          { choice: { kind: 'system' }, path: 'git', version: null, problem: 'too old' },
          {
            choice: { kind: 'custom', path: '/usr/bin/git' },
            path: '/usr/bin/git',
            version: '2.43.0',
            problem: null,
          },
        ],
        inUse: 'git',
        inUseVersion: null,
      },
    };
  }

  it('still opens the page that can point Coral at a different git', async () => {
    // The whole reason that page exists. It used to be rendered inside the branch that needs a
    // loaded graph, so the one machine that needed it was the one machine that could not
    // reach it.
    wire(tooOld());
    const view = render(App);
    await waitFor(() => {
      if (!view.container.textContent?.includes('too old')) throw new Error('no error yet');
    });

    await fireEvent.click(view.getByLabelText('Settings'));

    // The page arrives before its answer does, so wait for the answer: what matters is that a
    // usable git is offered, not that the heading rendered.
    await waitFor(() => {
      if (!view.container.textContent?.includes('2.43.0: /usr/bin/git')) {
        throw new Error('no git offered yet');
      }
    });
  });

  it('offers only the settings that mean anything without a repository', async () => {
    // SSH keys and commit signing are per-repository and have nothing to read. Listing them
    // would be offering two dead ends beside the one thing that works.
    wire(tooOld());
    const view = render(App);
    await waitFor(() => {
      if (!view.container.textContent?.includes('too old')) throw new Error('no error yet');
    });

    await fireEvent.click(view.getByLabelText('Settings'));
    await waitFor(() => {
      if (!view.container.textContent?.includes('Experimental')) throw new Error('not yet');
    });

    const panes = [...view.container.querySelectorAll('nav .pane')].map((b) => b.textContent);
    expect(panes.join(' ')).toContain('Experimental');
    expect(panes.join(' ')).not.toContain('SSH');
  });
});

describe('the settings page and the tab strip', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  it('follows the tab, rather than showing the repository you have left', async () => {
    /**
     * Two of the four panes are about one repository. Nothing reloaded them, so clicking a tab
     * with settings open left the ssh key and the signing configuration of the repository you
     * had just navigated away from on screen, with the tab strip underneath already showing
     * the other one. There is no way to tell from the page which of the two it is describing,
     * which is why the pane also names the repository now.
     */
    const other = '/srv/other';
    const both = {
      tabs: [
        { id: 1, path: REPO, group: null },
        { id: 2, path: other, group: null },
      ],
      groups: [],
    };
    const asked: string[] = [];
    const table = answers({
      session_get: { ...both, active: 1 },
      // The window opens the repository it was started on, which answers with the session
      // again; without this the default single-tab answer would replace both.
      tab_open: { ...both, active: 1 },
      tab_activate: { ...both, active: 2 },
      ssh_read: sshScopes(),
      ssh_keys: [],
      signing_read: signingScopes(),
      experimental_git: { chosen: { kind: 'system' }, candidates: [], effective: '/usr/bin/git' },
    });
    invoke.mockImplementation(async (cmd: string, args: { path?: string } = {}) => {
      if (cmd === 'ssh_read') asked.push(args.path ?? '');
      if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
      return table[cmd];
    });

    const view = render(App);
    await waitFor(() => {
      if (view.container.querySelectorAll('li.row').length === 0) throw new Error('no rows yet');
    });
    await fireEvent.click(view.getByLabelText('Settings'));
    await waitFor(() => {
      if (asked.length === 0) throw new Error('the ssh pane has not loaded');
    });
    expect(asked.at(-1), 'the repository it was opened on').toBe(REPO);

    const tabs = [...view.container.querySelectorAll('nav.bar .tab .pick')];
    await fireEvent.click(tabs[1] as HTMLElement);

    await waitFor(() => {
      if (asked.at(-1) !== other) throw new Error('still on the first repository');
    });
    // And it says which, so nobody has to infer it from the tab strip behind the page.
    await waitFor(() => {
      if (!view.container.querySelector('.prefs')?.textContent?.includes('other')) {
        throw new Error('the pane does not name the repository');
      }
    });
  });
});

describe('changing profile', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  const BLANK = {
    user: { name: null, email: null },
    ssh: { privateKey: null, publicKey: null, credentialHelper: null },
    signing: { format: null, program: null, key: null, signCommits: null, signTags: null },
  };

  function registry(current: string) {
    return {
      current,
      profiles: [
        { id: 'personal', name: 'Personal', colour: 'lane1', settings: BLANK },
        { id: 'work', name: 'Work', colour: 'lane3', settings: BLANK },
      ],
    };
  }

  /**
   * A profile's colour is how it is recognised: it is the dot on the chip in the title strip
   * and the dot beside its name in the settings list. The one list it was missing from was the
   * menu that chip opens — the list you choose a profile from.
   */
  it('marks each profile in the switcher with its own colour', async () => {
    const view = await shell({ profile_list: registry('personal') });
    const chip = await waitFor(() => {
      const found = view.container.querySelector('.strip .chip') as HTMLElement | null;
      if (!found) throw new Error('no chip yet');
      return found;
    });
    await fireEvent.click(chip);

    const named = [...view.container.querySelectorAll('.menu button')].filter((b) =>
      /Personal|Work/u.test(b.textContent ?? ''),
    );
    expect(named.length).toBe(2);
    for (const row of named) {
      const swatch = row.querySelector('.swatch') as HTMLElement | null;
      expect(swatch, `${row.textContent?.trim()} has a swatch`).not.toBeNull();
      expect(swatch?.style.background).toMatch(/^var\(--lane-\d\)$/u);
    }
  });

  it('names the profile in the title strip', async () => {
    const view = await shell({ profile_list: registry('work') });
    await waitFor(() => {
      if (!view.container.querySelector('.strip .chip')?.textContent?.includes('Work')) {
        throw new Error('the chip does not name it yet');
      }
    });
  });

  it('walks the graph again even when both profiles were left on the same repository', async () => {
    /**
     * The trap this test exists for. The window loads a repository when the tab's path differs
     * from the one already loaded, so two profiles left on the same path would swap every tab
     * and never load anything: the sidebar, the graph and the watcher would all still belong
     * to the tab that had just been closed, while the strip showed the other profile's.
     */
    const other = { tabs: [{ id: 9, path: REPO, group: null }], groups: [], active: 9 };
    const table = answers({
      profile_list: registry('personal'),
      profile_switch: { registry: registry('work'), session: other, recents: [] },
    });
    let walks = 0;
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'open_repo') walks += 1;
      if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
      return table[cmd];
    });

    const view = render(App);
    await waitFor(() => {
      if (view.container.querySelectorAll('li.row').length === 0) throw new Error('no rows yet');
    });
    const opened = walks;

    await fireEvent.click(view.container.querySelector('.strip .chip') as HTMLElement);
    const entries = [...view.container.querySelectorAll('.menu button')];
    const work = entries.find((b) => b.textContent?.includes('Work'));
    expect(work, 'the menu lists the other profile').not.toBeUndefined();
    await fireEvent.click(work as HTMLElement);

    await waitFor(() => {
      if (!view.container.querySelector('.strip .chip')?.textContent?.includes('Work')) {
        throw new Error('still in the old profile');
      }
    });
    await waitFor(() => {
      if (walks <= opened) throw new Error('the repository was never opened again');
    });
  });
});

describe('a repository that will not open', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  /**
   * The loading screen shows while there are no rows yet, which is nearly always because the
   * walk is running. It must not show when the walk is never going to start: a window saying
   * "reading the repository" under a banner explaining that it could not be read has hung, as
   * far as anyone looking at it is concerned.
   */
  it('says why instead of loading forever', async () => {
    wire({ open_repo: new Error('not a git repository') });
    const { container } = render(App);

    await waitFor(() => {
      const banner = container.querySelector('.banner.error');
      if (!banner) throw new Error('no banner yet');
      expect(banner.textContent).toContain('not a git repository');
    });
    expect(container.querySelector('.splash')).toBeNull();
  });
});

describe('what an action leaves behind', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  /** The same branch, on a different commit, so a fetch counts as a real change. */
  function spikeOn(row: number) {
    return [
      {
        name: 'refs/heads/spike',
        short: 'spike',
        kind: { kind: 'local_branch' },
        target: frameOids[row],
        peeled: null,
        upstream: null,
        ahead: 0,
        behind: 0,
        row,
      },
    ];
  }

  const fetched = { repo_action: { what: 'fetch', message: '', conflicted: false } };

  /**
   * The row a ref carries belongs to the walk it was read from. An action reads the refs
   * before the walk, because that is the only way to tell whether anything moved and a rewalk
   * is needed at all — and if it stops there, one commit fetched in shifts every row and
   * leaves every label a commit behind. Which is what the window did.
   */
  it('reads the refs again after the walk, not only before it', async () => {
    const { container } = await shell({ ...fetched, repo_refs: spikeOn(1) });
    wire({ ...fetched, repo_refs: spikeOn(2) });
    invoke.mockClear();

    const fetch = [...container.querySelectorAll('button.action')].find(
        (b) => b.getAttribute('aria-label') === 'Fetch',
      ) as HTMLButtonElement;
    expect(fetch, 'the toolbar carries a fetch button').toBeDefined();
    await fireEvent.click(fetch);

    await waitFor(() => {
      const order = invoke.mock.calls.map(([cmd]) => cmd as string);
      const walked = order.lastIndexOf('graph_rewalk');
      const read = order.lastIndexOf('repo_refs');
      if (walked === -1) throw new Error(`no rewalk: ${order.join(',')}`);
      expect(read).toBeGreaterThan(walked);
    });
  });

  /**
   * After a fetch that moved a ref, the rows on screen are a different set of commits, not a
   * stale copy of this one — so the graph takes the arriving path, which paints the fast
   * commit-time frame first.
   *
   * Kept, it took the reloading path: no first paint, and the exact walk repaints only when it
   * is done. Pulling four months of the kernel therefore left the old tip on screen saying
   * nothing for the fifty seconds that took, and closing the tab was the only way anyone found
   * to see the new commits.
   */
  it('paints the fast frame first when a fetch moved something', async () => {
    const { container } = await shell({ ...fetched, repo_refs: spikeOn(1) });
    wire({ ...fetched, repo_refs: spikeOn(2) });
    invoke.mockClear();

    await fireEvent.click(
      [...container.querySelectorAll('button.action')].find(
        (b) => b.getAttribute('aria-label') === 'Fetch',
      ) as HTMLButtonElement,
    );

    await waitFor(() => {
      const frames = invoke.mock.calls
        .filter(([cmd]) => cmd === 'graph_frame')
        .map(([, args]) => (args as { firstPaint?: boolean }).firstPaint);
      if (frames.length === 0) throw new Error('no frame asked for yet');
      expect(frames, 'the fast frame, then the exact one').toContain(true);
      expect(frames).toContain(false);
    });
  });

  /** A ref that has not moved must not cost a walk of the whole repository. */
  it('does not rewalk when nothing moved', async () => {
    const { container } = await shell({ ...fetched, repo_refs: spikeOn(1) });
    wire({ ...fetched, repo_refs: spikeOn(1) });
    invoke.mockClear();

    await fireEvent.click(
      [...container.querySelectorAll('button.action')].find(
        (b) => b.getAttribute('aria-label') === 'Fetch',
      ) as HTMLButtonElement,
    );

    await waitFor(() => {
      if (!invoke.mock.calls.some(([cmd]) => cmd === 'repo_action')) throw new Error('not yet');
    });
    expect(invoke.mock.calls.some(([cmd]) => cmd === 'graph_rewalk')).toBe(false);
  });
});

describe('the labels a row cannot fit', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  /** `n` refs all on the same commit: one local branch and the rest tracking branches. */
  function crowded(n: number) {
    const remotes = ['origin', 'github', 'gitlab', 'mirror', 'backup'];
    const refs = [
      {
        name: 'refs/heads/main',
        short: 'main',
        kind: { kind: 'local_branch' },
        target: frameOids[1],
        peeled: null,
        upstream: null,
        ahead: 0,
        behind: 0,
        row: 1,
      },
    ];
    for (let i = 0; i < n - 1; i += 1) {
      const remote = remotes[i % remotes.length];
      refs.push({
        name: `refs/remotes/${remote}/main`,
        short: `${remote}/main`,
        kind: { kind: 'remote_branch' },
        target: frameOids[1],
        peeled: null,
        upstream: null,
        ahead: 0,
        behind: 0,
        row: 1,
      });
    }
    return refs;
  }

  /**
   * A row holding a branch and its tracking branches is one branch in several places. A count
   * says how many and not which, so up to three the chip carries their marks instead.
   */
  it('shows a mark for each label it hides, while there are few enough to read', async () => {
    const { container } = await shell({ repo_refs: crowded(3) });
    const chip = await waitFor(() => {
      const found = container.querySelector('button.more');
      if (!found) throw new Error('no chip yet');
      return found;
    });
    expect(chip.querySelectorAll('svg')).toHaveLength(2);
    expect(chip.textContent?.trim()).toBe('');
    // The marks are decorative, so the name has to come from somewhere a reader can use.
    expect(chip.getAttribute('aria-label')).toContain('origin/main');
    expect(chip.getAttribute('aria-label')).toContain('github/main');
  });

  /** Past three, marks stop being read at a glance and the count says more than they do. */
  it('counts instead once there are more marks than can be taken in', async () => {
    const { container } = await shell({ repo_refs: crowded(6) });
    const chip = await waitFor(() => {
      const found = container.querySelector('button.more');
      if (!found) throw new Error('no chip yet');
      return found;
    });
    expect(chip.querySelectorAll('svg')).toHaveLength(0);
    expect(chip.textContent?.trim()).toBe('+5');
  });

  it('opens the rest of them when it is clicked', async () => {
    const { container } = await shell({ repo_refs: crowded(3) });
    const chip = (await waitFor(() => {
      const found = container.querySelector('button.more');
      if (!found) throw new Error('no chip yet');
      return found;
    })) as HTMLElement;

    await fireEvent.click(chip);
    const labels = [...container.querySelectorAll('.menu .label')].map((e) =>
      e.textContent?.trim(),
    );
    expect(labels.some((l) => l?.includes('origin/main'))).toBe(true);
  });
});

describe("the webview's own context menu", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  /**
   * Right-clicking where this window has no menu of its own used to bring up the webview's:
   * Back, Forward, Reload, and Inspect Element in a debug build. Reload restarts the
   * interface and takes a half-written commit message with it.
   */
  it('is refused where Coral has nothing of its own to show', async () => {
    const { container } = await shell();
    const body = container.querySelector('main') as HTMLElement;

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    body.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });

  /** Except in a field, where it is the only way to reach the clipboard. */
  it('is left alone inside a text field', async () => {
    const { container } = await shell();
    const field = container.querySelector('input') as HTMLInputElement;
    expect(field, 'the window has a field to test with').toBeTruthy();

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    field.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });
});

describe('moving down the list with the keyboard', () => {
  /**
   * The row overlay is a click target, not a place to arrive at.
   *
   * Left in the tab order it put a focus ring on whichever row was last clicked, and `j` and
   * `k` then moved the highlight away from it — two rows claiming to be the current one, the
   * ring drawn as a rule right across the pane because the overlay is wider than the pane is.
   * It was also a million tab stops, every one of them announced as "Select commit".
   *
   * Nothing is lost: the list is walked with j, k, the arrows, Home and End, all bound on the
   * window rather than on anything focused, and the selected row is what says where you are.
   */
  it('leaves no ring behind on the row the pointer last used', async () => {
    const { container } = await shell();
    const hit = container.querySelector('li.row button.hit') as HTMLButtonElement;
    expect(hit.getAttribute('tabindex')).toBe('-1');

    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText);
    const quiet = rules.find((text) => /\.hit[^{]*:focus-visible/u.test(text));
    expect(quiet).toMatch(/box-shadow:\s*none/u);
  });

  it('still selects the row it is clicked on', async () => {
    const { container } = await shell();
    const rows = [...container.querySelectorAll('li.row')];
    await fireEvent.click(rows[1]?.querySelector('button.hit') as HTMLButtonElement);
    expect(rows[1]?.classList.contains('selected')).toBe(true);
  });
});
