// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it, vi } from 'vitest';

// Each test mounts its own sidebar. Without this the previous one is still in the document and
// a query for a name that both rendered finds two of it.
afterEach(cleanup);

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import Sidebar from '../../src/app/Sidebar.svelte';
import FileTree from '../../src/app/FileTree.svelte';
import { buildTree } from '../../src/diff/tree';
import type { PlacedRef } from '../../src/ipc/commands';
import type { PullRequest } from '../../src/ipc/commands';
import type { ChangedFile, Submodule } from '../../src/ipc/types';

function ref(short: string, row: number | null = 0): PlacedRef {
  return {
    name: `refs/heads/${short}`,
    short,
    kind: { kind: 'local_branch' },
    target: 'a'.repeat(40),
    peeled: null,
    upstream: null,
    ahead: 0,
    behind: 0,
    row,
  };
}

function tag(short: string): PlacedRef {
  return { ...ref(short), name: `refs/tags/${short}`, kind: { kind: 'tag', annotated: false } };
}

function stash(name: string, oid: string, row: number | null = 0) {
  return {
    index: 0,
    oid,
    branch: 'master',
    message: 'WIP on master: 1a2b3c4 a commit',
    time: 1_756_000_000,
    name,
    row,
  };
}

function mount(over: Record<string, unknown> = {}) {
  return render(Sidebar, {
    props: {
      groups: { local: [ref('master')], remote: [], tags: [], stashes: [] },
      head: 'master',
      stashes: [],
      submodules: [],
      pullRequests: [],
      pullRequestLabel: 'Pull requests',
      remotes: [],
      openSubmodule: null,
      onSelect: () => {},
      onOpenSubmodule: () => {},
      onDropRef: () => {},
      onOpenPullRequest: () => {},
      onRemoteMenu: () => {},
      collapsed: {},
      onCollapse: () => {},
      onInitAllSubmodules: () => {},
      onSubmoduleMenu: () => {},
      onStashMenu: () => {},
      onRefMenu: () => {},
      detachedHead: null,
      ...over,
    },
  });
}

describe('the sidebar', () => {
  it('says where HEAD is when it is on no branch, and goes there when asked', async () => {
    // Checking a commit out detaches HEAD at it, and the branch list is then a list of
    // branches HEAD is not on: without this there is nothing saying where it went.
    const went: number[] = [];
    const { container } = mount({
      head: null,
      detachedHead: { oid: 'c0ffee1234567890', row: 42 },
      onSelect: (row: number) => went.push(row),
    });

    const local = [...container.querySelectorAll('section')].find((sec) =>
      sec.textContent?.includes('Local'),
    );
    const head = [...(local?.querySelectorAll('button.ref') ?? [])].find((b) =>
      b.textContent?.includes('HEAD'),
    ) as HTMLButtonElement;
    expect(head).toBeTruthy();
    expect(head.textContent).toContain('c0ffee12');

    await fireEvent.click(head);
    expect(went).toEqual([42]);
  });

  it('offers no way to HEAD when its commit is outside the loaded graph', () => {
    const { container } = mount({ detachedHead: { oid: 'c0ffee1234567890', row: null } });
    const head = [...container.querySelectorAll('button.ref')].find((b) =>
      b.textContent?.includes('HEAD'),
    ) as HTMLButtonElement;
    expect(head.disabled).toBe(true);
  });

  it('says nothing about HEAD while it is on a branch', () => {
    expect(mount().container.textContent).not.toContain('HEAD');
  });

  it('caps a long list but lets the cap be lifted', async () => {
    // The kernel carries 944 tags; the ones past the cap were unreachable.
    const tags = Array.from({ length: 944 }, (_, i) => tag(`v${i}`));
    const { container } = mount({ groups: { local: [], remote: [], tags, stashes: [] } });

    const section = [...container.querySelectorAll('section')].find((s) =>
      s.textContent?.includes('Tags'),
    );
    await fireEvent.click(section?.querySelector('button.head') as HTMLButtonElement);
    expect(section?.querySelectorAll('button.ref')).toHaveLength(200);

    const more = section?.querySelector('button.more') as HTMLButtonElement;
    expect(more.textContent).toContain('944');
    await fireEvent.click(more);
    expect(section?.querySelectorAll('button.ref')).toHaveLength(944);
  });

  it('shows submodules only when there are some, and opens an initialised one', async () => {
    expect(mount().container.textContent).not.toContain('Submodules');

    const opened: string[] = [];
    const submodules: Submodule[] = [
      { name: 'dev-scripts', path: 'external/dev-scripts', url: 'g://x', pinned: 'abc', initialised: true },
      { name: 'other', path: 'vendor/other', url: 'g://y', pinned: null, initialised: false },
    ];
    const { container } = mount({ submodules, onOpenSubmodule: (p: string) => opened.push(p) });

    const section = [...container.querySelectorAll('section')].find((s) =>
      s.textContent?.includes('Submodules'),
    );
    const rows = [...(section?.querySelectorAll('.row') ?? [])] as HTMLElement[];
    expect(rows).toHaveLength(2);
    // One that has never been cloned is marked, but the mark is all it is.
    expect(rows[1]?.querySelector('.ref')?.className).toContain('absent');

    // The row is not a control. Opening is one of four things that can be done with a
    // submodule, and making it the one a click performs hides the other three.
    await fireEvent.click(rows[0]?.querySelector('.ref') as HTMLElement);
    expect(opened).toEqual([]);
  });

  it('offers the submodule menu from the dots and from a right-click', async () => {
    const asked: string[] = [];
    const submodules: Submodule[] = [
      { name: 'dev', path: 'external/dev-scripts', url: 'g://x', pinned: 'abc', initialised: true },
    ];
    const { container } = mount({
      submodules,
      onSubmoduleMenu: (_e: MouseEvent, s: Submodule) => asked.push(s.path),
    });

    // Scoped to the submodules section. Remotes grew rows of their own, and an unscoped
    // `section .row` finds the remote section's header first.
    const section = [...container.querySelectorAll('section')].find((el) =>
      el.textContent?.includes('Submodules'),
    );
    const row = section?.querySelector('.row') as HTMLElement;
    await fireEvent.click(row.querySelector('.dots') as HTMLElement);
    expect(asked).toEqual(['external/dev-scripts']);

    await fireEvent.contextMenu(row.closest('li') as HTMLElement);
    expect(asked).toEqual(['external/dev-scripts', 'external/dev-scripts']);
  });

  it('shows proposals only when the host gave some, and opens one', async () => {
    expect(mount().container.textContent).not.toContain('Pull requests');

    const opened: PullRequest[] = [];
    const pullRequests: PullRequest[] = [
      {
        number: 42,
        title: 'Add a thing',
        state: 'draft',
        author: 'alice',
        sourceBranch: 'feature/x',
        targetBranch: 'main',
        webUrl: 'https://gitlab.com/g/p/-/merge_requests/42',
        updatedAt: '2026-01-01T00:00:00Z',
      },
    ];
    const { container } = mount({
      pullRequests,
      pullRequestLabel: 'Merge requests',
      onOpenPullRequest: (pr: PullRequest) => opened.push(pr),
    });

    const section = [...container.querySelectorAll('section')].find((s) =>
      s.textContent?.includes('Merge requests'),
    );
    expect(section).toBeDefined();
    const row = section?.querySelector('button.pr') as HTMLButtonElement;
    expect(row.textContent).toContain('#42');
    expect(row.querySelector('.state')?.className).toContain('draft');
    await fireEvent.click(row);
    expect(opened[0]?.number).toBe(42);
  });

  it('reports a branch dropped onto another', async () => {
    const drops: [string, string][] = [];
    const { container } = mount({
      groups: { local: [ref('master'), ref('feature')], remote: [], tags: [], stashes: [] },
      onDropRef: (a: string, b: string) => drops.push([a, b]),
    });
    const rows = [...container.querySelectorAll('button.ref')] as HTMLButtonElement[];
    const transfer = { setData: () => {}, effectAllowed: '', dropEffect: '' };
    await fireEvent.dragStart(rows[1] as HTMLElement, { dataTransfer: transfer });
    await fireEvent.dragOver(rows[0] as HTMLElement, { dataTransfer: transfer });
    await fireEvent.drop(rows[0] as HTMLElement, { dataTransfer: transfer });
    expect(drops).toEqual([['feature', 'master']]);
  });

  it('ignores a branch dropped on itself', async () => {
    const drops: [string, string][] = [];
    const { container } = mount({ onDropRef: (a: string, b: string) => drops.push([a, b]) });
    const row = container.querySelector('button.ref') as HTMLElement;
    const transfer = { setData: () => {}, effectAllowed: '', dropEffect: '' };
    await fireEvent.dragStart(row, { dataTransfer: transfer });
    await fireEvent.drop(row, { dataTransfer: transfer });
    expect(drops).toEqual([]);
  });
});

describe('the file tree', () => {
  const files: ChangedFile[] = [
    { path: 'drivers/net/ethernet/intel/ice/ice_main.c', oldPath: null, change: 'modified' },
    { path: 'README', oldPath: null, change: 'added' },
  ];

  it('collapses a deep chain into one row and opens the file under it', async () => {
    const opened: string[] = [];
    const { container } = render(FileTree, {
      props: { nodes: buildTree(files), openPath: null, onOpenFile: (p: string) => opened.push(p) },
    });
    expect(container.querySelector('button.dir')?.textContent).toContain(
      'drivers/net/ethernet/intel/ice',
    );
    const file = [...container.querySelectorAll('button.file')].find((b) =>
      b.textContent?.includes('ice_main.c'),
    );
    await fireEvent.click(file as HTMLButtonElement);
    expect(opened).toEqual(['drivers/net/ethernet/intel/ice/ice_main.c']);
  });

  it('hides the contents of a directory when it is collapsed', async () => {
    const { container } = render(FileTree, {
      props: { nodes: buildTree(files), openPath: null, onOpenFile: () => {} },
    });
    expect(container.textContent).toContain('ice_main.c');
    await fireEvent.click(container.querySelector('button.dir') as HTMLButtonElement);
    expect(container.textContent).not.toContain('ice_main.c');
    // The file at the root is not inside it and must stay.
    expect(container.textContent).toContain('README');
  });
});

describe('the stash list', () => {
  it('lists the whole stack, not the one ref git keeps for it', () => {
    // `refs/stash` is the top of the stack and the only stash with a ref, so listing refs found
    // one stash however many there were — and called it "stash", which is also what it called
    // the next one.
    const view = mount({
      stashes: [
        stash('master@78d0a2d', '78d0a2dd1a00b8e06286b02bac7ca3811a7041fc'),
        stash('master@21b55eb', '21b55ebccffb3a1b09c67b16ef4b009cce430e5e'),
      ],
    });
    expect(view.getByText('master@78d0a2d')).toBeTruthy();
    expect(view.getByText('master@21b55eb')).toBeTruthy();
  });

  it('offers what can be done with one from the dots and from a right-click', () => {
    const asked: string[] = [];
    const view = mount({
      stashes: [stash('master@78d0a2d', 'a'.repeat(40))],
      onStashMenu: (_e: MouseEvent, s: { name: string }) => asked.push(s.name),
    });

    const dots = view.getByTitle('What can be done with master@78d0a2d');
    void fireEvent.click(dots);
    void fireEvent.contextMenu(view.getByText('master@78d0a2d'));

    expect(asked).toEqual(['master@78d0a2d', 'master@78d0a2d']);
  });

  it('will not send you to a stash it cannot show you', () => {
    // A stash whose commit is outside the loaded walk has no row to reveal. Clicking one used
    // to do nothing at all, with nothing to say why.
    const picked: number[] = [];
    const view = mount({
      stashes: [stash('master@78d0a2d', 'a'.repeat(40), null)],
      onSelect: (row: number) => picked.push(row),
    });

    const row = view.getByText('master@78d0a2d').closest('button');
    expect(row?.hasAttribute('disabled')).toBe(true);
    void fireEvent.click(row as HTMLElement);
    expect(picked).toEqual([]);
  });

  it('says so when there is nothing stashed', () => {
    expect(mount().getByText('Nothing stashed.')).toBeTruthy();
  });
});
