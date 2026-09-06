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
    },
    repo_conflicts: [],
    hosting_status: { host: null, detail: 'no remotes', signedIn: false },
    hosting_pull_requests: [],
    session_get: { tabs: [{ id: 1, path: REPO, group: null }], active: 1, groups: [] },
    tab_open: { tabs: [{ id: 1, path: REPO, group: null }], active: 1, groups: [] },
    tab_activate: { tabs: [{ id: 1, path: REPO, group: null }], active: 1, groups: [] },
    ...over,
  };
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
      (b) => b.textContent?.includes('Patch'),
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
      [...container.querySelectorAll('button.action')].find((b) =>
        b.textContent?.includes('Patch'),
      ) as HTMLButtonElement;
    await waitFor(() => {
      if (!patchButton().title.includes('Write')) throw new Error('not comparing yet');
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
      },
      repo_conflicts: [{ path: 'shared.txt', kind: 'both_modified' }],
    });

    await waitFor(() => {
      if (!container.textContent?.includes('merge in progress')) throw new Error('not stopped yet');
    });
    expect(container.textContent).not.toContain('Select a commit');
    expect(container.querySelector('aside.wip-panel')).toBeNull();
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
      },
      repo_conflicts: [
        { path: 'f.txt', kind: 'both_modified', binary: false, deleteModify: false },
      ],
    });
    await waitFor(() => {
      if (!container.querySelector('section.merge')) throw new Error('no merge tool');
    });
    expect((container.querySelector('.graph') as HTMLElement).className).toContain('hidden');
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
