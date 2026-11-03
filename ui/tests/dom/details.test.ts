// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent, waitFor } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import Details from '../../src/app/Details.svelte';
import { SECTION_LIMITS, SectionsState } from '../../src/state/sections.svelte';
import { tick } from 'svelte';
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
  // A getter, so every spread of these props gets a panel layout of its own.
  get sections() {
    return new SectionsState();
  },
};

describe('the commit panel', () => {
  beforeEach(() => localStorage.clear());

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

  it('shows a signature only on a signed commit, with git\'s reason when it could not check', () => {
    const plain = render(Details, { props: { ...props, detail: detail() } });
    expect(plain.container.textContent).not.toContain('Signature');
    plain.unmount();

    const reason = 'gpg.ssh.allowedSignersFile needs to be configured';
    const { container } = render(Details, {
      props: { ...props, detail: detail(), verification: { kind: 'unchecked', reason } },
    });
    const row = container.querySelector('dd.signature');
    expect(row?.textContent).toContain('Signed, but git could not check it here');
    expect(row?.textContent).toContain(reason);
    expect(row?.classList.contains('warn')).toBe(true);
  });

  it('shows a commit\'s note, with a way to edit it', async () => {
    const edit = vi.fn();
    const { container } = render(Details, {
      props: { ...props, detail: detail(), note: 'Reviewed by Ada', onEditNote: edit },
    });
    expect(container.querySelector('.note')?.textContent).toContain('Reviewed by Ada');
    await fireEvent.click(container.querySelector('.edit-note') as HTMLElement);
    expect(edit).toHaveBeenCalledOnce();
  });

  it('shows no note section for a commit without one', () => {
    const { container } = render(Details, { props: { ...props, detail: detail() } });
    expect(container.querySelector('.note')).toBeNull();
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

  it('offers a folder in the tree its own menu, apart from its files', async () => {
    // A build folder committed by mistake is one thing to stop tracking, not every file in it.
    const files: ChangedFile[] = [{ path: 'build/out.bin', oldPath: null, change: 'added' }];
    const onFolderMenu = vi.fn();
    const onFileMenu = vi.fn();
    const { container } = render(Details, {
      props: { ...props, grouping: 'tree' as const, detail: detail(files), onFolderMenu, onFileMenu },
    });

    await fireEvent.contextMenu(container.querySelector('button.dir') as HTMLElement);
    expect(onFolderMenu).toHaveBeenCalledWith(expect.anything(), 'build');
    expect(onFileMenu).not.toHaveBeenCalled();
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

describe('the commit panel sections', () => {
  beforeEach(() => localStorage.clear());

  const files: ChangedFile[] = [
    { path: 'kernel/sched/core.c', oldPath: null, change: 'modified' },
    { path: 'kernel/sched/fair.c', oldPath: null, change: 'modified' },
  ];
  const divider = '[role="separator"][aria-label="Resize the commit message and its files"]';

  function withBody(): CommitDetail {
    const d = detail(files);
    return { ...d, commit: { ...d.commit, body: 'Pull scheduler fixes.\n\nThree of them.' } };
  }

  function fold(container: HTMLElement, name: RegExp): HTMLButtonElement {
    const button = [...container.querySelectorAll<HTMLButtonElement>('button.fold')].find((b) =>
      name.test(b.textContent ?? ''),
    );
    if (!button) throw new Error(`no fold for ${name}`);
    return button;
  }

  it('scrolls the message and the files apart, with a divider between them', () => {
    const { container } = render(Details, { props: { ...props, detail: withBody() } });
    const head = container.querySelector('.head');
    const scroll = container.querySelector('.scroll');
    expect(head?.querySelector('.body')).not.toBeNull();
    expect(scroll?.querySelector('.files')).not.toBeNull();
    expect(head?.contains(scroll ?? null)).toBe(false);
    expect(container.querySelector(divider)).not.toBeNull();
    expect(head?.classList.contains('shared')).toBe(true);

    // happy-dom does not lay out, so the scrolling is read from the rules.
    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText);
    for (const part of ['head', 'scroll']) {
      const rule = rules.find(
        (text) => new RegExp(`\\.${part}[^{,]*\\{`, 'u').test(text) && /overflow-y:\s*auto/u.test(text),
      );
      expect(rule, `.${part} scrolls on its own`).toBeDefined();
    }
  });

  it('folds the message to its summary and gives the files the panel', async () => {
    const { container } = render(Details, { props: { ...props, detail: withBody() } });
    const summary = fold(container, /Merge tag something/u);
    await fireEvent.click(summary);
    expect(summary.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('.body')).toBeNull();
    expect(container.querySelector('dl')).toBeNull();
    expect(container.querySelector('#detail-message')).toBeNull();
    expect(container.querySelector(divider)).toBeNull();
    expect(container.querySelector('.head')?.classList.contains('closed')).toBe(true);
    expect(container.querySelectorAll('.files li')).toHaveLength(2);
  });

  it('folds the files to their count and gives the message the panel', async () => {
    const { container } = render(Details, { props: { ...props, detail: withBody() } });
    const heading = fold(container, /2 files/u);
    await fireEvent.click(heading);
    expect(heading.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('.files')).toBeNull();
    expect(container.querySelector('.filebar')).toBeNull();
    expect(container.querySelector(divider)).toBeNull();
    expect(container.querySelector('.head')?.classList.contains('shared')).toBe(false);
    expect(container.querySelector('.body')).not.toBeNull();
  });

  it('sizes the message from its divider and resets on a double-click', async () => {
    const sections = new SectionsState();
    const { container } = render(Details, { props: { ...props, sections, detail: withBody() } });
    const handle = container.querySelector<HTMLElement>(divider);
    const head = container.querySelector<HTMLElement>('.head');
    expect(head?.style.height).toBe('');

    // Down makes the message taller: it is anchored to the top of the panel.
    if (handle) await fireEvent.keyDown(handle, { key: 'ArrowDown', shiftKey: true });
    expect(sections.sizes.messageHeight).toBe(SECTION_LIMITS.messageHeight.min);
    sections.resize('messageHeight', 220);
    await tick();
    expect(head?.style.height).toBe('220px');
    expect(head?.classList.contains('sized')).toBe(true);

    if (handle) await fireEvent.dblClick(handle);
    expect(sections.sizes.messageHeight).toBeNull();
    expect(head?.style.height).toBe('');
  });

  it('keeps a fold from one commit to the next', async () => {
    const sections = new SectionsState();
    const { container, rerender } = render(Details, {
      props: { ...props, sections, detail: withBody() },
    });
    await fireEvent.click(fold(container, /Merge tag something/u));
    const next = withBody();
    await rerender({ detail: { ...next, commit: { ...next.commit, oid: 'f'.repeat(40), summary: 'Another' } } });
    expect(fold(container, /Another/u).getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('.body')).toBeNull();
  });

  it('folds the comparison the same way', async () => {
    const { container } = render(Details, {
      props: {
        ...props,
        detail: null,
        compare: { from: 'a'.repeat(40), to: 'b'.repeat(40), files },
      },
    });
    await fireEvent.click(fold(container, /Comparing two commits/u));
    expect(container.querySelector('.single')).toBeNull();
    expect(container.querySelectorAll('.files li')).toHaveLength(2);
  });

  it('has nothing to fold while there is no commit', () => {
    const { container } = render(Details, { props: { ...props, detail: null } });
    expect(container.querySelector('button.fold')).toBeNull();
    expect(container.querySelector('[role="separator"]')).toBeNull();
  });
});
