// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * The title bar Coral draws for itself.
 *
 * The window is opened without decorations, which means the buttons that minimise, maximise
 * and close it are ordinary elements in the page. If they are missing, or the strip stops
 * offering the way back to the desktop's own title bar, a window manager that handles an
 * undecorated window badly leaves someone with a window they cannot close.
 */
const frameBytes = readFileSync(resolve(process.cwd(), 'tests/fixtures/frame.bin'));

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const win = vi.hoisted(() => ({
  decorated: false,
  maximized: false,
  /** What the window was last told, so a click can be checked against it. */
  calls: [] as string[],
  resized: null as (() => void) | null,
}));

vi.mock('../../src/ipc/window', () => ({
  minimize: async () => void win.calls.push('minimize'),
  startDragging: async () => void win.calls.push('startDragging'),
  toggleMaximize: async () => void win.calls.push('toggleMaximize'),
  close: async () => void win.calls.push('close'),
  startResize: async (at: string) => void win.calls.push(`resize:${at}`),
  isMaximized: async () => win.maximized,
  isDecorated: async () => win.decorated,
  setDecorations: async (on: boolean) => {
    win.calls.push(`setDecorations:${String(on)}`);
    win.decorated = on;
  },
  onResized: async (run: () => void) => {
    win.resized = run;
    return () => (win.resized = null);
  },
}));

import App from '../../src/app/App.svelte';

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

async function shell() {
  const table = answers();
  invoke.mockImplementation(async (cmd: string) => {
    if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
    return table[cmd];
  });
  const view = render(App);
  await waitFor(() => {
    if (view.container.querySelectorAll('li.row').length === 0) throw new Error('no rows yet');
  });
  return view;
}

function named(container: HTMLElement, label: string): HTMLElement | null {
  return container.querySelector(`.strip button[aria-label="${label}"]`);
}

beforeEach(() => {
  localStorage.clear();
  invoke.mockReset();
  win.decorated = false;
  win.maximized = false;
  win.calls = [];
  win.resized = null;
});

describe('the title bar Coral draws', () => {
  it('is one strip carrying the tabs, the tools and the window buttons', async () => {
    const { container } = await shell();
    const strip = container.querySelector('.strip');
    expect(strip, 'the strip').not.toBeNull();
    // The tabs live in it rather than on a row of their own, which is the whole point.
    expect(strip?.querySelector('nav.bar'), 'the tabs').not.toBeNull();
    for (const label of ['Settings', 'Switch theme']) {
      expect(named(container, label), label).not.toBeNull();
    }
    // The log is reached from the corner of the status bar instead, beside the work it
    // reports on rather than beside the buttons that close the window.
    expect(named(container, 'Activity logs'), 'not in the strip').toBeNull();
    expect(container.querySelector('.status .logs'), 'in the status bar').not.toBeNull();
  });

  it('minimises, maximises and closes the window', async () => {
    const { container } = await shell();
    await waitFor(() => {
      if (!named(container, 'Minimise')) throw new Error('not yet undecorated');
    });

    await fireEvent.click(named(container, 'Minimise') as HTMLElement);
    await fireEvent.click(named(container, 'Maximise') as HTMLElement);
    await fireEvent.click(named(container, 'Close') as HTMLElement);
    expect(win.calls).toEqual(['minimize', 'toggleMaximize', 'close']);
  });

  it('moves the window from the bare strip, and never from a control', async () => {
    const { container } = await shell();
    const strip = container.querySelector('.strip') as HTMLElement;

    await fireEvent.mouseDown(strip, { button: 0, clientX: 200, clientY: 12 });
    await fireEvent.mouseMove(window, { clientX: 260, clientY: 40 });
    expect(win.calls, 'the bare strip moves it').toEqual(['startDragging']);
    await fireEvent.mouseUp(window);

    // The empty stretch of the tab strip is the same surface and moves the window too. The
    // tabs are dragged with the pointer themselves, so neither a tab nor a button may.
    win.calls = [];
    for (const at of ['nav.bar', '.tab .pick'] as const) {
      await fireEvent.mouseDown(container.querySelector(at) as HTMLElement, {
        button: 0,
        clientX: 200,
        clientY: 12,
      });
      await fireEvent.mouseMove(window, { clientX: 260, clientY: 40 });
      await fireEvent.mouseUp(window);
    }
    await fireEvent.mouseDown(named(container, 'Settings') as HTMLElement, { button: 0 });
    await fireEvent.mouseMove(window, { clientX: 260, clientY: 40 });
    await fireEvent.mouseUp(window);
    expect(win.calls, 'the bare stretch, and nothing else').toEqual(['startDragging']);
  });

  /**
   * A press that never moves is a click, and a click on the strip must do nothing to the
   * window. Handing the pointer to the window manager on the press itself made every click a
   * drag of zero distance, and a window manager that snaps a drag near the top of the screen
   * answered that by maximising — so clicking the strip, which is at the top of the screen by
   * definition, threw the window full size.
   */
  it('does nothing at all when the press does not move', async () => {
    const { container } = await shell();
    const strip = container.querySelector('.strip') as HTMLElement;

    await fireEvent.mouseDown(strip, { button: 0, clientX: 200, clientY: 12 });
    await fireEvent.mouseUp(window, { clientX: 200, clientY: 12 });
    expect(win.calls, 'a click leaves the window alone').toEqual([]);

    // Nor does a hand that wobbles a pixel or two on the way back up.
    await fireEvent.mouseDown(strip, { button: 0, clientX: 200, clientY: 12 });
    await fireEvent.mouseMove(window, { clientX: 202, clientY: 13 });
    await fireEvent.mouseUp(window, { clientX: 202, clientY: 13 });
    expect(win.calls).toEqual([]);
  });

  it('stops listening once the press is over, so a later move is not a drag', async () => {
    const { container } = await shell();
    const strip = container.querySelector('.strip') as HTMLElement;

    await fireEvent.mouseDown(strip, { button: 0, clientX: 200, clientY: 12 });
    await fireEvent.mouseUp(window, { clientX: 200, clientY: 12 });
    await fireEvent.mouseMove(window, { clientX: 400, clientY: 300 });
    expect(win.calls).toEqual([]);
  });

  it('hands the pointer over exactly once, however far it goes', async () => {
    const { container } = await shell();
    const strip = container.querySelector('.strip') as HTMLElement;

    await fireEvent.mouseDown(strip, { button: 0, clientX: 200, clientY: 12 });
    await fireEvent.mouseMove(window, { clientX: 260, clientY: 40 });
    await fireEvent.mouseMove(window, { clientX: 300, clientY: 80 });
    await fireEvent.mouseMove(window, { clientX: 360, clientY: 120 });
    expect(win.calls).toEqual(['startDragging']);
    await fireEvent.mouseUp(window);
  });

  it('never starts a window drag from a panel\'s backdrop', async () => {
    // The tab drawer and the icon picker put their dismiss backdrop inside the strip, because
    // that is where the component that owns them is mounted. A backdrop is not a control, so
    // the strip used to treat a press on it as a press on itself and hand the pointer to the
    // window manager — which never delivers the click, so the panel stayed open however far
    // outside it you pressed. Asserted as "no drag" rather than "it closed": a mocked window
    // swallows nothing, so the click lands in this test either way and only the drag shows
    // the fault.
    const { container } = await shell();
    await fireEvent.click(container.querySelector('.tail .find') as HTMLElement);
    const scrim = container.querySelector('.strip .scrim') as HTMLElement;
    expect(scrim, 'the drawer is open on its backdrop').not.toBeNull();

    win.calls = [];
    await fireEvent.mouseDown(scrim, { button: 0 });
    expect(win.calls, 'a backdrop is not the strip').toEqual([]);

    await fireEvent.click(scrim);
    expect(container.querySelector('.finder'), 'and the drawer closed').toBeNull();
  });

  it('leaves a double click alone, rather than maximising on it', async () => {
    const { container } = await shell();
    const strip = container.querySelector('.strip') as HTMLElement;
    // Tauri's own drag region maximises on the second press. This strip is not one, which is
    // the only thing keeping a stray double click off the whole screen.
    expect(strip.getAttribute('data-tauri-drag-region'), 'not Tauri\'s drag region').toBeNull();

    await fireEvent.mouseDown(strip, { button: 0, detail: 2 });
    expect(win.calls).not.toContain('toggleMaximize');
  });

  it('shows the path on the repository crumb, not in the strip', async () => {
    const { container } = await shell();
    expect(container.querySelector('.strip .path'), 'no path in the strip').toBeNull();
    const crumb = [...container.querySelectorAll('.where .step')].find((s) =>
      s.querySelector('.label')?.textContent?.trim() === 'repository',
    );
    expect(crumb?.getAttribute('title')).toBe(REPO);
  });

  it('offers to restore once the window is maximised', async () => {
    const { container } = await shell();
    await waitFor(() => {
      if (!named(container, 'Maximise')) throw new Error('not yet undecorated');
    });

    win.maximized = true;
    win.resized?.();
    await waitFor(() => {
      if (!named(container, 'Restore')) throw new Error('still says maximise');
    });
    expect(named(container, 'Maximise'), 'no longer offers to maximise').toBeNull();
  });

  it('draws the edges the desktop no longer draws, and resizes from one', async () => {
    const { container } = await shell();
    await waitFor(() => {
      if (container.querySelectorAll('.grip').length === 0) throw new Error('no grips yet');
    });
    // Four edges and four corners.
    expect(container.querySelectorAll('.grip')).toHaveLength(8);

    const corner = container.querySelector('.grip.southeast') as HTMLElement;
    await fireEvent.mouseDown(corner, { button: 0 });
    expect(win.calls).toEqual(['resize:SouthEast']);
  });

  it('hands the window back to the desktop, and remembers that it did', async () => {
    const { container } = await shell();
    await waitFor(() => {
      if (!named(container, 'Close')) throw new Error('not yet undecorated');
    });

    const strip = container.querySelector('.strip') as HTMLElement;
    await fireEvent.contextMenu(strip);
    const item = [...container.querySelectorAll('.menu .label')].find(
      (e) => e.textContent?.trim() === 'Use the system title bar',
    );
    expect(item, 'the way back').toBeDefined();
    await fireEvent.click((item as HTMLElement).closest('button') as HTMLElement);

    await waitFor(() => {
      if (named(container, 'Close')) throw new Error('still drawing its own buttons');
    });
    expect(win.calls).toContain('setDecorations:true');
    // No grips either: the desktop's own border is back.
    expect(container.querySelectorAll('.grip')).toHaveLength(0);
    expect(localStorage.getItem('coral.views')).toContain('"systemTitleBar":true');
  });

  it('draws no buttons of its own where the desktop draws the title bar', async () => {
    win.decorated = true;
    const { container } = await shell();
    await waitFor(() => {
      if (!container.querySelector('.strip')) throw new Error('no strip');
    });
    for (const label of ['Minimise', 'Maximise', 'Close']) {
      expect(named(container, label), label).toBeNull();
    }
    expect(container.querySelectorAll('.grip')).toHaveLength(0);
  });
});
