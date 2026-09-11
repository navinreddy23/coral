// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent, waitFor } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import Details from '../../src/app/Details.svelte';
import { authorColourIndex } from '../../src/graph/initials';
import type { ChangedFile, CommitDetail } from '../../src/ipc/types';

function detail(files: ChangedFile[] = []): CommitDetail {
  const who = { name: 'Linus Torvalds', email: 'torvalds@linux-foundation.org', time: 1_756_000_000 };
  return {
    commit: {
      oid: '89a312991dc6e638a36adc43ccb91dbc25504c049a58da80053f'.slice(0, 40),
      parents: ['4aa2c106aef4aaaaaaaaaaaaaaaaaaaaaaaaaaaa'],
      author: who,
      committer: who,
      summary: 'Merge tag something',
      body: '',
    },
    files,
  };
}

const props = {
  repo: '/repo',
  compare: null,
  nothing: false,
  loading: false,
  error: null,
  openPath: null,
  grouping: 'path' as const,
  onGrouping: () => {},
  onOpenFile: () => {},
  onClearCompare: () => {},
  onCopied: () => {},
};

describe('the commit panel', () => {
  it('gives an author the colour the graph gives them', () => {
    // Both discs are the author's initials on one of eight fills, chosen by hashing who they
    // are — but the graph hashed the email and this hashed the name, so the same person came
    // out purple in the row and mustard in the panel that answers it. The point of the disc is
    // that the eye recognises it without reading a name, and two colours is no colour at all.
    const d = detail();
    const { container } = render(Details, { props: { ...props, detail: d } });
    const face = container.querySelector('.face') as HTMLElement;

    const index = authorColourIndex(d.commit.author.email.trim().toLowerCase(), 8);
    expect(face.style.background).toBe(`var(--node-${index + 1})`);
  });

  it('gives an author with no email a colour rather than none', () => {
    const d = detail();
    d.commit.author = { ...d.commit.author, email: '  ' };
    const { container } = render(Details, { props: { ...props, detail: d } });
    const face = container.querySelector('.face') as HTMLElement;

    const index = authorColourIndex(d.commit.author.name, 8);
    expect(face.style.background).toBe(`var(--node-${index + 1})`);
  });

  it('gives each term exactly one description', () => {
    // A second dd for the same term flows back into the label column of the two-column grid,
    // where an unbreakable object id sets the width and squeezes the values out.
    const { container } = render(Details, { props: { ...props, detail: detail() } });
    const list = container.querySelector('dl');
    expect(list).not.toBeNull();

    let terms = 0;
    let descriptions = 0;
    for (const child of list?.children ?? []) {
      if (child.tagName === 'DT') {
        // Every dt must be immediately followed by exactly one dd.
        expect(descriptions).toBe(terms);
        terms += 1;
      } else if (child.tagName === 'DD') {
        descriptions += 1;
      }
    }
    expect(terms).toBeGreaterThan(0);
    expect(descriptions).toBe(terms);
  });

  it('keeps every parent inside the one description', () => {
    const d = detail();
    d.commit.parents = ['a'.repeat(40), 'b'.repeat(40)];
    const { container } = render(Details, { props: { ...props, detail: d } });
    const dds = [...(container.querySelector('dl')?.querySelectorAll('dd') ?? [])];
    const parents = dds.filter((dd) => dd.textContent?.includes('aaaa'));
    expect(parents).toHaveLength(1);
    expect(parents[0]?.textContent).toContain('bbbb');
  });

  it('makes every file a button, so a diff can be opened from it', () => {
    const files: ChangedFile[] = [
      { path: 'kernel/sched/core.c', oldPath: null, change: 'modified' },
      { path: 'README', oldPath: null, change: 'added' },
    ];
    const { container } = render(Details, { props: { ...props, detail: detail(files) } });
    const buttons = [...container.querySelectorAll('ul.files button')];
    expect(buttons).toHaveLength(2);
    expect(buttons[0]?.textContent).toContain('core.c');
  });

  it('marks the file whose diff is open', () => {
    const files: ChangedFile[] = [{ path: 'a.c', oldPath: null, change: 'modified' }];
    const { container } = render(Details, {
      props: { ...props, detail: detail(files), openPath: 'a.c' },
    });
    expect(container.querySelector('ul.files button')?.className).toContain('open');
  });
});

describe('counting the files a comparison turns up', () => {
  /**
   * The verb was appended whatever the count, so a comparison that turned up one file read
   * "1 file differ".
   */
  function heading(count: number): string {
    const files: ChangedFile[] = Array.from({ length: count }, (_, i) => ({
      path: `f${i}.txt`,
      oldPath: null,
      change: 'modified',
    }));
    const { container } = render(Details, {
      props: { ...props, detail: null, compare: { from: 'a'.repeat(40), to: 'b'.repeat(40), files } },
    });
    return container.querySelector('h3')?.textContent?.trim() ?? '';
  }

  it('agrees with itself about one file', () => {
    expect(heading(1)).toBe('1 file differs');
  });

  it('and about several', () => {
    expect(heading(3)).toBe('3 files differ');
  });

  it('and about none', () => {
    expect(heading(0)).toBe('0 files differ');
  });
});

describe('counting everything at a commit', () => {
  /** The same slip the other way round: "1 files at this commit". */
  it('counts one file as one file', async () => {
    const { invoke } = await import('../../src/ipc/invoke');
    vi.mocked(invoke).mockResolvedValue(['log.txt']);
    const { container } = render(Details, { props: { ...props, detail: detail() } });

    const box = container.querySelector('label.all input') as HTMLInputElement;
    await fireEvent.click(box);
    await waitFor(() => {
      if (!container.querySelector('.count')) throw new Error('no count yet');
    });
    expect(container.querySelector('.count')?.textContent?.trim()).toBe('1 file at this commit');
  });
});
