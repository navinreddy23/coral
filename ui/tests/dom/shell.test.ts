// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

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

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import App from '../../src/app/App.svelte';
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
    row_metadata: Array.from({ length: 256 }, (_, i) => ({
      oid: `${i}`.padStart(40, '0'),
      author: 'Linus Torvalds',
      email: 'torvalds@linux-foundation.org',
      time: 1_756_000_000,
      summary: `commit number ${i}`,
      body: '',
    })),
    repo_refs: [],
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
    return table[cmd];
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
    for (const rule of painting) expect(rule, rule).toContain('cell.message');
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
