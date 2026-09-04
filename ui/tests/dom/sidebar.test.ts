// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn() }));
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

function mount(over: Record<string, unknown> = {}) {
  return render(Sidebar, {
    props: {
      groups: { local: [ref('master')], remote: [], tags: [], stashes: [] },
      head: 'master',
      submodules: [],
      pullRequests: [],
      pullRequestLabel: 'Pull requests',
      onSelect: () => {},
      onOpenSubmodule: () => {},
      onDropRef: () => {},
      onOpenPullRequest: () => {},
      ...over,
    },
  });
}

describe('the sidebar', () => {
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
    const rows = [...(section?.querySelectorAll('button.ref') ?? [])] as HTMLButtonElement[];
    expect(rows).toHaveLength(2);
    // One that has never been cloned has nothing to open.
    expect(rows[1]?.disabled).toBe(true);
    await fireEvent.click(rows[0] as HTMLButtonElement);
    expect(opened).toEqual(['external/dev-scripts']);
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
