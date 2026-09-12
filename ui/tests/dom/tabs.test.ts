// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { decodeFrame, oidOf } from '../../src/graph/frame';

/**
 * Two repositories in two tabs, and what has to happen when the user moves between them.
 *
 * The state modules are reloaded per repository, but several are not: a commit selected in one
 * repository, a diff opened from it, a scroll position. None of that means anything in the
 * other one, and showing it there is showing another repository's contents under this
 * repository's name.
 */
const frameBytes = readFileSync(resolve(process.cwd(), 'tests/fixtures/frame.bin'));
const frameBuffer = () =>
  frameBytes.buffer.slice(frameBytes.byteOffset, frameBytes.byteOffset + frameBytes.byteLength);

/**
 * The object ids the fixture frame actually holds.
 *
 * Metadata is keyed by object id, because a row is a position in one walk and the walk is
 * replaced whenever the repository moves. A stub that answered with invented ids would be
 * answering about commits that are not on those rows.
 */
const frameOids = (() => {
  const frame = decodeFrame(frameBuffer());
  return Array.from({ length: frame.rowCount }, (_, r) => oidOf(frame, r));
})();

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import App from '../../src/app/App.svelte';

const A = '/repos/alpha';
const B = '/repos/beta';

/** A session with both tabs, active as given. */
function session(active: number) {
  return {
    tabs: [
      { id: 1, path: A, group: null },
      { id: 2, path: B, group: null },
    ],
    active,
    groups: [],
  };
}

let active = 1;

function wire() {
  active = 1;
  invoke.mockImplementation(async (cmd: string, args: Record<string, unknown> = {}) => {
    const path = String(args['path'] ?? '');
    switch (cmd) {
      case 'initial_repo':
        return A;
      case 'session_get':
        return session(active);
      case 'tab_activate':
        active = Number(args['id']);
        return session(active);
      case 'tab_open':
      case 'tab_close':
      case 'tab_enter_submodule':
      case 'tab_leave_submodule':
        return session(active);
      case 'open_repo':
        return {
          path,
          gitDir: `${path}/.git`,
          gitVersion: '2.43.0',
          head: { kind: 'branch', name: path === A ? 'alpha-main' : 'beta-main' },
          state: 'clean',
          commitGraph: true,
        };
      case 'binary_self_test':
        return Uint8Array.from({ length: 4096 }, (_, i) => i % 251).buffer;
      case 'graph_frame':
        return frameBuffer();
      case 'row_metadata':
        return frameOids.map((oid, i) => ({
          oid,
          author: 'A',
          email: 'a@b.c',
          time: 1_756_000_000,
          summary: `${path === A ? 'alpha' : 'beta'} commit ${i}`,
          body: '',
        }));
      case 'commit_detail':
        return {
          commit: {
            oid: String(args['rev'] ?? ''),
            parents: [],
            author: { name: 'A', email: 'a@b.c', time: 1_756_000_000 },
            committer: { name: 'A', email: 'a@b.c', time: 1_756_000_000 },
            summary: `${path === A ? 'alpha' : 'beta'} detail`,
            body: '',
          },
          files: [{ path: 'one.txt', oldPath: null, change: 'modified' }],
        };
      case 'file_diff':
        return {
          path: 'one.txt',
          oldPath: null,
          change: 'modified',
          binary: false,
          added: 1,
          removed: 0,
          tooLarge: false,
          hunks: [],
        };
      case 'terminal_open':
        return { id: path === A ? 11 : 22, shell: '/bin/sh' };
      case 'terminal_write':
      case 'terminal_resize':
      case 'terminal_close':
        return null;
      case 'repo_status':
        return { entries: [], conflicted: [] };
      case 'repo_operation':
        return {
          state: 'clean',
          labels: { ours: '', theirs: '', swapped: false },
          progress: null,
          headName: null,
          stoppedAt: null,
          interactive: false,
          resumable: true,
        };
      case 'hosting_status':
        return { host: null, detail: 'no remotes', token: 'none' };
      case 'repo_submodules':
        return path === A
          ? [
              {
                name: 'lib/berkeley-db',
                path: 'lib/berkeley-db',
                url: 'https://example.com/db.git',
                pinned: '0'.repeat(40),
                initialised: true,
              },
              {
                name: 'lib/never-cloned',
                path: 'lib/never-cloned',
                url: 'https://example.com/n.git',
                pinned: '1'.repeat(40),
                initialised: false,
              },
            ]
          : [];
      default:
        return [];
    }
  });
}

async function shell() {
  wire();
  const view = render(App);
  await waitFor(() => {
    if (view.container.querySelectorAll('li.row').length === 0) throw new Error('no rows');
  });
  return view;
}

/** Clicks the tab whose label matches. */
async function switchTo(container: HTMLElement, label: string) {
  const tab = [...container.querySelectorAll('nav .tab button.pick')].find((b) =>
    b.textContent?.includes(label),
  );
  expect(tab, `a tab labelled ${label}`).toBeDefined();
  await fireEvent.click(tab as HTMLButtonElement);
}

describe('two repositories in two tabs', () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  it('opens both and shows the active one', async () => {
    const { container } = await shell();
    expect(container.querySelectorAll('nav .tab')).toHaveLength(2);
    await waitFor(() => {
      if (!container.textContent?.includes('alpha commit')) throw new Error('not alpha yet');
    });
    expect(container.querySelector('.status')?.textContent).toContain('alpha-main');
  });

  it('loads the other repository when its tab is picked', async () => {
    const { container } = await shell();
    await switchTo(container, 'beta');
    await waitFor(() => {
      if (!container.textContent?.includes('beta commit')) throw new Error('not beta yet');
    });
    expect(container.querySelector('.status')?.textContent).toContain('beta-main');
    // And the branch it left is no longer on screen anywhere.
    expect(container.textContent).not.toContain('alpha commit');
  });

  it('does not carry a selected commit across', async () => {
    // The detail panel would otherwise describe a commit from another repository under this
    // repository's name.
    const { container } = await shell();
    const row = container.querySelector('li.row button.hit') as HTMLButtonElement;
    await fireEvent.click(row);
    await waitFor(() => {
      if (!container.textContent?.includes('alpha detail')) throw new Error('no detail yet');
    });

    await switchTo(container, 'beta');
    await waitFor(() => {
      if (!container.textContent?.includes('beta commit')) throw new Error('not beta yet');
    });
    expect(container.textContent).not.toContain('alpha detail');
    // The sidebar is an <aside> too; the detail panel is the last one.
    const panels = [...container.querySelectorAll('aside')];
    expect(panels.at(-1)?.textContent).toContain('Select a commit');
  });

  it('does not leave another repository’s diff open', async () => {
    const { container } = await shell();
    await fireEvent.click(container.querySelector('li.row button.hit') as HTMLButtonElement);
    await waitFor(() => {
      if (!container.querySelector('ul.files button')) throw new Error('no files yet');
    });
    await fireEvent.click(container.querySelector('ul.files button') as HTMLButtonElement);
    await waitFor(() => {
      if (!container.querySelector('section.diff')) throw new Error('no diff yet');
    });

    await switchTo(container, 'beta');
    await waitFor(() => {
      if (!container.textContent?.includes('beta commit')) throw new Error('not beta yet');
    });
    expect(container.querySelector('section.diff')).toBeNull();
  });

  it('shows a submodule inside the tab that declares it, not in a tab of its own', async () => {
    // A submodule belongs to the repository that declares it, at the commit that repository
    // records. A second tab loses that relationship and leaves two entries in the bar with no
    // way to tell which came from which.
    const { container } = await shell();
    const section = await waitFor(() => {
      const found = [...container.querySelectorAll('section')].find((s) =>
        s.textContent?.includes('Submodules'),
      );
      if (!found) throw new Error('no submodules section');
      return found;
    });

    const rows = [...section.querySelectorAll('.row')] as HTMLElement[];
    expect(rows).toHaveLength(2);

    // Through the menu, since the row itself is not a control.
    await fireEvent.click(rows[0]?.querySelector('.dots') as HTMLElement);
    const open = [...document.querySelectorAll('.menu .label')].find(
      (e) => e.textContent?.trim() === 'Open this submodule',
    ) as HTMLElement;
    expect(open, 'the menu offers to open it').toBeTruthy();
    await fireEvent.click(open);

    await waitFor(() => {
      const entered = invoke.mock.calls.filter(([cmd]) => cmd === 'tab_enter_submodule').at(-1);
      if (!entered) throw new Error('no submodule entered');
      expect(entered[1]).toEqual({ id: 1, path: 'lib/berkeley-db' });
    });
    // And no tab was opened for it.
    const opened = invoke.mock.calls.filter(([cmd]) => cmd === 'tab_open');
    expect(opened.every(([, args]) => (args as { path: string }).path !== `${A}/lib/berkeley-db`))
      .toBe(true);
  });

  it('starts the other repository at the top of its history', async () => {
    const { container } = await shell();
    const scroller = container.querySelector('.graph') as HTMLElement;
    scroller.scrollTop = 4000;
    await fireEvent.scroll(scroller);

    await switchTo(container, 'beta');
    await waitFor(() => {
      if (!container.textContent?.includes('beta commit')) throw new Error('not beta yet');
    });
    // Row 900 of one history is not row 900 of another.
    expect(scroller.scrollTop).toBe(0);
  });

  it('opens the terminal in the repository the tab is showing', async () => {
    // The shell was started once for the window, in whichever repository was open first, and
    // every other tab then showed a prompt sitting in that checkout.
    const { container } = await shell();
    await waitFor(() => {
      const opened = invoke.mock.calls.filter(([cmd]) => cmd === 'terminal_open');
      if (opened.length === 0) throw new Error('no shell yet');
    });
    expect(
      invoke.mock.calls
        .filter(([cmd]) => cmd === 'terminal_open')
        .map(([, args]) => (args as { path: string }).path),
    ).toEqual([A]);

    await switchTo(container, 'beta');
    await waitFor(() => {
      const paths = invoke.mock.calls
        .filter(([cmd]) => cmd === 'terminal_open')
        .map(([, args]) => (args as { path: string }).path);
      if (!paths.includes(B)) throw new Error('no shell in the second repository');
    });
  });

  it('keeps each repository’s shell, rather than starting another on every visit', async () => {
    // The working directory is the point, but so is a half-typed command: coming back to a tab
    // has to come back to the same shell.
    const { container } = await shell();
    await switchTo(container, 'beta');
    await waitFor(() => {
      if (!container.textContent?.includes('beta commit')) throw new Error('not beta yet');
    });
    await switchTo(container, 'alpha');
    await waitFor(() => {
      if (!container.textContent?.includes('alpha commit')) throw new Error('not alpha yet');
    });

    const paths = invoke.mock.calls
      .filter(([cmd]) => cmd === 'terminal_open')
      .map(([, args]) => (args as { path: string }).path);
    expect(paths.filter((p) => p === A)).toHaveLength(1);
    expect(paths.filter((p) => p === B)).toHaveLength(1);
    expect(invoke.mock.calls.filter(([cmd]) => cmd === 'terminal_close')).toHaveLength(0);
  });

  it('keeps the pane of a repository it is not showing, rather than rebuilding it', async () => {
    // Rebuilding the pane per tab starts no second shell, but xterm holds the scrollback and
    // the subscription to the shell's output — so a rebuilt pane comes back blank in front of
    // a shell that is still running.
    const { container } = await shell();
    await switchTo(container, 'beta');
    await waitFor(() => {
      if (container.querySelectorAll('.term .pane').length < 2) throw new Error('one pane');
    });

    const panes = [...container.querySelectorAll('.term .pane')] as HTMLElement[];
    expect(panes).toHaveLength(2);
    expect(panes.filter((p) => !p.hidden)).toHaveLength(1);
  });
  it('stops showing the last repository the moment its tab is left', async () => {
    // Opening a repository is a read, and on a kernel-sized one it is long enough to stand and
    // look at. The rows used to be dropped only when that read came back, so the tab, the
    // crumb and the branch said one repository while the list, the lanes and "1,482,171
    // commits" were still the one just left. The loading screen asks whether there are rows,
    // so leaving them up is what kept it from appearing.
    const { container } = await shell();
    await waitFor(() => {
      if (!container.textContent?.includes('alpha commit')) throw new Error('not alpha yet');
    });

    // Beta's repository read never comes back, which is the window the fault lived in.
    const answering = invoke.getMockImplementation();
    if (!answering) throw new Error('no stub');
    invoke.mockImplementation(async (cmd: string, args: Record<string, unknown> = {}) => {
      if (cmd === 'open_repo' && String(args['path'] ?? '') === B) {
        return new Promise(() => {});
      }
      return answering(cmd, args);
    });

    await switchTo(container, 'beta');
    await waitFor(() => {
      if (container.textContent?.includes('alpha commit')) throw new Error('alpha still up');
    });
    expect(container.querySelectorAll('li.row')).toHaveLength(0);
    expect(container.textContent).toContain('Reading the repository');
  });
});
