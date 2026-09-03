// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import Details from '../../src/app/Details.svelte';
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

const props = { loading: false, error: null, openPath: null, onOpenFile: () => {} };

describe('the commit panel', () => {
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
