// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import DiffView from '../../src/app/DiffView.svelte';
import { DiffState } from '../../src/state/diff.svelte';
import type { FileDiff, Line, LineKind } from '../../src/ipc/types';
import { ViewsState } from '../../src/state/views.svelte';

function line(kind: LineKind, text: string, oldNo: number | null, newNo: number | null): Line {
  return { kind, text, oldNo, newNo, noNewline: false };
}

function fileDiff(): FileDiff {
  return {
    path: 'kernel/sched/core.c',
    oldPath: null,
    change: 'modified',
    binary: false,
    added: 1,
    removed: 1,
    tooLarge: false,
    hunks: [
      {
        header: '@@ -1,3 +1,3 @@',
        oldStart: 1,
        oldLines: 3,
        newStart: 1,
        newLines: 3,
        lines: [
          line('context', 'one', 1, 1),
          line('remove', 'two', 2, null),
          line('add', 'TWO', null, 2),
          line('context', 'three', 3, 3),
        ],
      },
    ],
  };
}

function mounted(mode: 'inline' | 'split') {
  const diff = new DiffState(new ViewsState());
  diff.path = 'kernel/sched/core.c';
  diff.file = fileDiff();
  diff.setMode(mode);
  return render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
}

describe('the diff viewer', () => {
  it('shows one row per line inline, with both numberings', () => {
    const { container } = mounted('inline');
    const rows = [...container.querySelectorAll('tbody tr')].filter(
      (r) => !r.classList.contains('hunk'),
    );
    expect(rows).toHaveLength(4);
    const added = rows.find((r) => r.classList.contains('add'));
    expect(added?.textContent).toContain('TWO');
    // The removed line keeps its old number and has no new one.
    const removed = rows.find((r) => r.classList.contains('remove'));
    const cells = [...(removed?.querySelectorAll('td.no') ?? [])].map((c) => c.textContent);
    expect(cells).toEqual(['2', '']);
  });

  it('puts the two sides opposite each other when split', () => {
    const { container } = mounted('split');
    const rows = [...container.querySelectorAll('.line')];
    // Context, the replaced pair, context: three rows, not four.
    expect(rows).toHaveLength(3);
    const middle = rows[1];
    const texts = [...(middle?.querySelectorAll('.cell') ?? [])].map((c) => c.textContent);
    expect(texts).toEqual(['two', 'TWO']);
  });

  it('builds only the rows the viewport can hold', () => {
    // A whole file is thousands of rows; a table that size is what made scrolling crawl.
    const lines = Array.from({ length: 4000 }, (_, i) => ({
      kind: 'context' as const,
      text: `line ${i}`,
      oldNo: i + 1,
      newNo: i + 1,
      noNewline: false,
    }));
    const diff = new DiffState(new ViewsState());
    diff.setMode('split');
    diff.path = 'big.txt';
    diff.file = {
      ...fileDiff(),
      path: 'big.txt',
      hunks: [{ header: '@@', oldStart: 1, oldLines: 4000, newStart: 1, newLines: 4000, lines }],
    };
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });

    const drawn = container.querySelectorAll('.line').length;
    expect(drawn).toBeGreaterThan(0);
    expect(drawn).toBeLessThan(200);
    // The sheet still stands for the whole file, so the scrollbar means what it says.
    const sheet = container.querySelector('.sheet');
    expect(sheet?.getAttribute('style')).toContain(`${4000 * 17}px`);
  });

  it('will not offer to blame a binary file', async () => {
    // git answers for one all the same, treating its bytes as lines, and the pane painted four
    // kilobytes of replacement characters. The diff beside it already says "Binary file".
    const diff = new DiffState(new ViewsState());
    diff.path = 'logo.bin';
    diff.file = { ...fileDiff(), path: 'logo.bin', binary: true, hunks: [] };
    const view = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });

    const blame = [...view.container.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Blame',
    ) as HTMLButtonElement;
    expect(blame.disabled).toBe(true);
    expect(blame.title).toContain('no lines to blame');
  });

  it('shows a blame that failed rather than waiting on it for ever', () => {
    const diff = new DiffState(new ViewsState());
    diff.path = 'vendor/sub';
    diff.file = fileDiff();
    diff.setView('blame');
    diff.sideError = 'fatal: no such path vendor/sub in HEAD';
    const view = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });

    expect(view.container.textContent).toContain('no such path vendor/sub');
    expect(view.container.textContent).not.toContain('Working out who wrote each line');
  });

  it('forgets which lines were picked when the diff is read again', async () => {
    // Staging part of a hunk reloads the same file in the same mode, and the hunks that come
    // back hold different lines under the same indices. Kept, the bar went on offering "Stage
    // 1 line" for a line nobody had picked, and git answered the patch built from it with
    // "corrupt patch at line 12".
    const diff = new DiffState(new ViewsState());
    diff.path = 'kernel/sched/core.c';
    diff.source = 'unstaged';
    diff.file = fileDiff();
    diff.setMode('inline');
    const view = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });

    await fireEvent.click(view.container.querySelector('button.pick') as HTMLElement);
    expect(view.getByText('Stage 1 line')).toBeTruthy();

    // The same path, the same mode, a fresh read: the picks belong to the rows that are gone.
    diff.file = fileDiff();
    await Promise.resolve();
    expect(view.queryByText('Stage 1 line')).toBeNull();
  });

  it('shows the hunk header inline, and none side by side', () => {
    const inline = mounted('inline').container;
    const headers = [...inline.querySelectorAll('tr.hunk')];
    expect(headers).toHaveLength(1);
    expect(headers[0]?.textContent).toContain('@@ -1,3 +1,3 @@');

    // Side by side asks for the file end to end, so there is one hunk covering all of it and
    // a header saying which lines it spans is noise.
    const split = mounted('split').container;
    expect(split.querySelectorAll('tr.hunk')).toHaveLength(0);
  });

  it('says so for a binary file rather than showing an empty table', () => {
    const diff = new DiffState(new ViewsState());
    diff.path = 'logo.png';
    diff.file = { ...fileDiff(), path: 'logo.png', binary: true, hunks: [], added: null, removed: null };
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    expect(container.textContent).toContain('Binary file');
    expect(container.querySelector('table')).toBeNull();
  });

  /**
   * git collapses an untracked directory to one entry — `? notes/` — rather than listing what
   * is inside it, which is what keeps `status` fast on a repository with a build tree in it.
   * Opening that entry answered "No line changes.", which is the opposite of what is true of a
   * directory full of new files.
   */
  it('says what an untracked directory is, rather than that nothing changed', () => {
    const diff = new DiffState(new ViewsState());
    diff.path = 'notes/';
    diff.file = { ...fileDiff(), path: 'notes/', change: 'added', hunks: [], added: 0, removed: 0 };
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    expect(container.textContent).toContain('A directory of new files');
    expect(container.textContent).not.toContain('No line changes');
  });

  it('still says nothing changed for a file that really did not', () => {
    const diff = new DiffState(new ViewsState());
    diff.path = 'notes/deep.md';
    diff.file = { ...fileDiff(), path: 'notes/deep.md', hunks: [], added: 0, removed: 0 };
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    expect(container.textContent).toContain('No line changes');
  });

  it('reports the error instead of a blank pane', () => {
    const diff = new DiffState(new ViewsState());
    diff.path = 'gone.c';
    diff.error = 'This commit did not change that file.';
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    expect(container.querySelector('.error')?.textContent).toContain('did not change');
  });
});

describe('a comparison of two commits', () => {
  /** The panel as a compare opens it, with the remembered view left on history. */
  function compared() {
    const views = new ViewsState();
    const diff = new DiffState(views);
    diff.setView('history');
    diff.setMode('inline');
    diff.path = 'kernel/sched/core.c';
    diff.file = fileDiff();
    diff.source = 'compare';
    return diff;
  }

  it('shows the change itself, whatever view the last file was left on', () => {
    // Blame and history are about one file's past. A range has no single revision to have
    // one, and the panel used to show a file's history beside the range's diff.
    const diff = compared();
    expect(diff.view).toBe('diff');
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    expect(container.querySelector('table.lines')).not.toBeNull();
    expect(container.querySelector('table.lines.blame')).toBeNull();
  });

  it('offers no blame or history tab for it', () => {
    const { container } = render(DiffView, {
      props: { diff: compared(), onClose: () => {}, onPart: () => {} },
    });
    const labels = [...container.querySelectorAll('header button')].map((b) => b.textContent?.trim());
    expect(labels).not.toContain('Blame');
    expect(labels).not.toContain('History');
  });

  it('leaves the remembered view alone, so the next file comes back to it', () => {
    const views = new ViewsState();
    const diff = new DiffState(views);
    diff.setView('blame');
    diff.source = 'compare';
    expect(diff.view).toBe('diff');
    diff.source = 'commit';
    expect(diff.view).toBe('blame');
  });
});

describe('the side-by-side layout', () => {
  it('gives each side half the width, whatever the lines hold', () => {
    // `1fr` is `minmax(auto, 1fr)`, so a column whose content cannot wrap — a long line under
    // `white-space: pre` — grows past its share and pushes everything after it along. That is
    // what put the right pane's gutter in the middle of the left pane's text and its code off
    // the side of the window.
    const diff = new DiffState(new ViewsState());
    diff.setMode('split');
    diff.path = 'kernel/sched/core.c';
    diff.file = fileDiff();
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });

    const line = container.querySelector('.line') as HTMLElement;
    const columns = getComputedStyle(line).gridTemplateColumns;
    // Both code columns, not one: the gutters are fixed and the two halves share the rest.
    expect(columns.match(/minmax\(0/gu) ?? []).toHaveLength(2);
  });
});

describe('reading a change in its surroundings', () => {
  it('offers to widen the unified view to the whole file, and to narrow it again', async () => {
    const { container } = mounted('inline');
    const wider = container.querySelector('button.wider') as HTMLButtonElement;
    expect(wider, 'the unified view offers it').not.toBeNull();
    expect(wider.textContent?.trim()).toBe('Whole file');

    await fireEvent.click(wider);
    expect(
      (container.querySelector('button.wider') as HTMLButtonElement).textContent?.trim(),
    ).toBe('Changes only');
  });

  it('does not offer it side by side, which already shows the whole file', () => {
    const { container } = mounted('split');
    expect(container.querySelector('button.wider')).toBeNull();
  });

  it('carries next and previous change in the unified view too', () => {
    const { container } = mounted('inline');
    const steps = [...container.querySelectorAll('.steps button')] as HTMLButtonElement[];
    expect(steps.length, 'two buttons').toBe(2);
    expect(steps.map((b) => b.title)).toEqual(['Previous change', 'Next change']);
    expect(steps.every((b) => !b.disabled), 'the fixture has changes to step to').toBe(true);
  });

  it('disables them for a file with nothing changed in it', () => {
    const diff = new DiffState(new ViewsState());
    diff.setMode('inline');
    diff.path = 'unchanged.txt';
    diff.file = {
      ...fileDiff(),
      hunks: [{ header: '@@', oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: [line('context', 'same', 1, 1)] }],
    };
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    const steps = [...container.querySelectorAll('.steps button')] as HTMLButtonElement[];
    expect(steps.every((b) => b.disabled)).toBe(true);
  });
});

describe('the words that changed inside a line', () => {
  /** A replaced line: one word differs, and the rest of it is the same on both sides. */
  function replaced(): FileDiff {
    return {
      ...fileDiff(),
      hunks: [
        {
          header: '@@ -1,3 +1,3 @@',
          oldStart: 1,
          oldLines: 3,
          newStart: 1,
          newLines: 3,
          lines: [
            line('context', 'fn main() {', 1, 1),
            line('remove', '    let total = count + 1;', 2, null),
            line('add', '    let total = amount + 1;', null, 2),
            line('context', '}', 3, 3),
          ],
        },
      ],
    };
  }

  function view(mode: 'inline' | 'split') {
    const diff = new DiffState(new ViewsState());
    diff.setMode(mode);
    diff.path = 'kernel/sched/core.c';
    diff.file = replaced();
    return render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
  }

  it('marks it in the unified table, and leaves the rest of the line alone', () => {
    const { container } = view('inline');
    const marks = [...container.querySelectorAll('tr.remove mark, tr.add mark')];
    expect(marks.map((m) => m.textContent)).toEqual(['count', 'amount']);

    // The whole line is still there: the marks are inside it, not instead of it.
    const removed = container.querySelector('tr.remove td.text') as HTMLElement;
    expect(removed.textContent).toContain('    let total = count + 1;');
  });

  it('marks it side by side too', () => {
    const { container } = view('split');
    const marks = [...container.querySelectorAll('.cell mark')];
    expect(marks.map((m) => m.textContent)).toEqual(['count', 'amount']);
  });

  it('takes the change map away in blame, where it would point at the wrong rows', () => {
    // The strip's marks, its "you are here" band and its jump all measure against the diff's
    // rows, and blame puts the whole file in the same scroller instead. On a file long enough
    // for the diff to be windowed those are different row sets, so the strip says the reader is
    // somewhere they are not and clicking it goes to the wrong line. History keeps it: the
    // diff is what it shows beside the commit list.
    expect(mounted('split').container.querySelector('.overview')).not.toBeNull();

    const diff = new DiffState(new ViewsState());
    diff.path = 'kernel/sched/core.c';
    diff.file = fileDiff();
    diff.setMode('split');
    diff.setView('blame');
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    expect(container.querySelector('.overview')).toBeNull();
  });

  it('takes the mark colour from the line it is on, which a scoped rule could not', () => {
    // The component that draws the mark is its own, so a rule written against `tr.add mark`
    // in this panel would never match it. The tint is handed down as a property instead.
    const { container } = view('inline');
    expect(container.querySelector('tr.add mark')).not.toBeNull();

    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText);
    expect(rules.some((text) => /mark[^{]*\{[^}]*var\(--word-mark/u.test(text))).toBe(true);
    expect(rules.some((text) => /--word-mark:\s*var\(--add-word\)/u.test(text))).toBe(true);
    expect(rules.some((text) => /--word-mark:\s*var\(--remove-word\)/u.test(text))).toBe(true);
  });

  it('marks nothing on a line that was rewritten rather than edited', () => {
    const diff = new DiffState(new ViewsState());
    diff.setMode('inline');
    diff.path = 'a.rs';
    diff.file = {
      ...fileDiff(),
      hunks: [
        {
          header: '@@',
          oldStart: 1,
          oldLines: 1,
          newStart: 1,
          newLines: 1,
          lines: [
            line('remove', 'let total = count + 1;', 1, null),
            line('add', 'emit(&mut out, "done")?;', null, 1),
          ],
        },
      ],
    };
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });
    expect(container.querySelectorAll('mark')).toHaveLength(0);
  });

  it('offers a way past the size guard, and asks again without it', async () => {
    // The comment on the guard in core promises "the UI offers an explicit load", and for a
    // while it did not: the panel said the contents were not read and stopped there.
    const diff = new DiffState(new ViewsState());
    diff.path = 'huge.txt';
    diff.file = { ...fileDiff(), hunks: [], tooLarge: true };
    const { container, getByText } = render(DiffView, {
      props: { diff, onClose: () => {}, onPart: () => {} },
    });
    expect(container.textContent).toContain('past the size guard');

    const asked = vi.spyOn(diff, 'readAnyway').mockResolvedValue(undefined);
    await fireEvent.click(getByText('Read it anyway'));
    expect(asked).toHaveBeenCalledOnce();
  });
  it('says which side has no newline at the end of the file', async () => {
    // git marks it and the panel dropped the flag, so a change that only added a trailing
    // newline drew "gamma" removed and "gamma" added with nothing saying what differs.
    const diff = new DiffState(new ViewsState());
    diff.setMode('inline');
    diff.path = 'nonewline.txt';
    diff.file = {
      ...fileDiff(),
      hunks: [
        {
          header: '@@ -1,3 +1,3 @@',
          oldStart: 1,
          oldLines: 3,
          newStart: 1,
          newLines: 3,
          lines: [
            line('context', 'alpha', 1, 1),
            line('context', 'beta', 2, 2),
            { ...line('remove', 'gamma', 3, null), noNewline: true },
            line('add', 'gamma', null, 3),
          ],
        },
      ],
    };
    const { container } = render(DiffView, { props: { diff, onClose: () => {}, onPart: () => {} } });

    const marked = [...container.querySelectorAll('tbody tr')].filter((r) =>
      r.querySelector('.nonl'),
    );
    expect(marked, 'only the side that lacks it').toHaveLength(1);
    expect(marked[0]?.classList.contains('remove')).toBe(true);
    expect(marked[0]?.querySelector('.nonl')?.getAttribute('title')).toContain('No newline');
  });
});
