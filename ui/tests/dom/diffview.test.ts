// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import DiffView from '../../src/app/DiffView.svelte';
import { DiffState } from '../../src/state/diff.svelte';
import type { FileDiff, Line, LineKind } from '../../src/ipc/types';

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
  const diff = new DiffState();
  diff.path = 'kernel/sched/core.c';
  diff.file = fileDiff();
  diff.mode = mode;
  return render(DiffView, { props: { diff, onClose: () => {} } });
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
    const rows = [...container.querySelectorAll('tbody tr')].filter(
      (r) => !r.classList.contains('hunk'),
    );
    // Context, the replaced pair, context: three rows, not four.
    expect(rows).toHaveLength(3);
    const middle = rows[1];
    const texts = [...(middle?.querySelectorAll('td.text') ?? [])].map((c) => c.textContent);
    expect(texts).toEqual(['two', 'TWO']);
  });

  it('shows the hunk header once in either mode', () => {
    for (const mode of ['inline', 'split'] as const) {
      const { container } = mounted(mode);
      const headers = [...container.querySelectorAll('tr.hunk')];
      expect(headers, mode).toHaveLength(1);
      expect(headers[0]?.textContent, mode).toContain('@@ -1,3 +1,3 @@');
    }
  });

  it('says so for a binary file rather than showing an empty table', () => {
    const diff = new DiffState();
    diff.path = 'logo.png';
    diff.file = { ...fileDiff(), path: 'logo.png', binary: true, hunks: [], added: null, removed: null };
    const { container } = render(DiffView, { props: { diff, onClose: () => {} } });
    expect(container.textContent).toContain('Binary file');
    expect(container.querySelector('table')).toBeNull();
  });

  it('reports the error instead of a blank pane', () => {
    const diff = new DiffState();
    diff.path = 'gone.c';
    diff.error = 'This commit did not change that file.';
    const { container } = render(DiffView, { props: { diff, onClose: () => {} } });
    expect(container.querySelector('.error')?.textContent).toContain('did not change');
  });
});
