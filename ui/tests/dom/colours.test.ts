// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The worker, run in this thread: the same library and the same reading of its answer.
vi.mock('../../src/syntax/highlighter', async () => {
  const { default: core } = await import('highlight.js/lib/core');
  const { default: c } = await import('highlight.js/lib/languages/c');
  const { tokensOf } = await import('../../src/syntax/token');
  core.registerLanguage('c', c);
  return {
    highlight: async (grammar: string, text: string) =>
      tokensOf(core.highlight(text, { language: grammar, ignoreIllegals: true }).value),
  };
});

import DiffView from '../../src/app/DiffView.svelte';
import { DiffState } from '../../src/state/diff.svelte';
import { SyntaxState } from '../../src/state/syntax.svelte';
import { ViewsState } from '../../src/state/views.svelte';
import type { FileDiff, Line, LineKind } from '../../src/ipc/types';

function line(kind: LineKind, text: string, oldNo: number | null, newNo: number | null): Line {
  return { kind, text, oldNo, newNo, noNewline: false };
}

const BEFORE = '/* start\n   of a comment */\nint a = 1;\n';
const AFTER = '/* start\n   of a comment */\nint a = 2;\n';

function fileDiff(): FileDiff {
  return {
    path: 'a.c',
    oldPath: null,
    change: 'modified',
    binary: false,
    added: 1,
    removed: 1,
    tooLarge: false,
    mode: null,
    hunks: [
      {
        header: '@@ -2,2 +2,2 @@',
        oldStart: 2,
        oldLines: 2,
        newStart: 2,
        newLines: 2,
        lines: [
          // Its text is the old side's, as under ignored whitespace: not this side's line 2.
          line('context', '  of a comment */', 2, 2),
          line('remove', 'int a = 1;', 3, null),
          line('add', 'int a = 2;', null, 3),
        ],
      },
    ],
  };
}

function mounted(mode: 'inline' | 'split', view: 'diff' | 'blame' | 'file' = 'diff') {
  const views = new ViewsState();
  const diff = new DiffState(views);
  const syntax = new SyntaxState(views);
  diff.path = 'a.c';
  diff.file = fileDiff();
  diff.setMode(mode);
  diff.setView(view);
  diff.sideTexts = { before: BEFORE, after: AFTER };
  // Read already: the test is about drawing what came back, not about reading it.
  diff.readSides = async () => {};
  return { views, diff, ...render(DiffView, { props: { diff, syntax, onClose: () => {}, onPart: () => {} } }) };
}

// The switch is remembered, and a test that turns it off must not leave it off for the next.
beforeEach(() => localStorage.clear());

describe('code in colour', () => {
  it('colours a diff line from its own side of the whole file, once the answer is back', async () => {
    const { container } = mounted('inline');
    await vi.waitFor(() => expect(container.querySelector('tr.add .tk-type')).not.toBeNull());
    expect(container.querySelector('tr.add .tk-type')?.textContent).toBe('int');
    expect(container.querySelector('tr.add .tk-constant')?.textContent).toBe('2');
    expect(container.querySelector('tr.remove .tk-constant')?.textContent).toBe('1');
  });

  it('leaves a line plain whose text is not the line its side has there', async () => {
    const { container } = mounted('inline');
    await vi.waitFor(() => expect(container.querySelector('tr.add .tk-type')).not.toBeNull());
    const context = container.querySelector('tr.context .text');
    expect(context?.querySelector('[class^="tk-"]')).toBeNull();
    expect(context?.textContent).toContain('of a comment */');
  });

  it('colours both columns side by side', async () => {
    const { container } = mounted('split');
    await vi.waitFor(() => expect(container.querySelector('.cell.add .tk-constant')).not.toBeNull());
    expect(container.querySelector('.cell.remove .tk-constant')?.textContent).toBe('1');
  });

  it('colours the file view and the blame by the file they show', async () => {
    const { diff, container } = mounted('inline', 'file');
    diff.contents = { kind: 'text', text: AFTER, before: false };
    await vi.waitFor(() => expect(container.querySelector('.file-sheet .tk-comment')).not.toBeNull());
    // The second line of the comment is a comment too, which it is only as part of the file.
    const second = container.querySelectorAll('.file-sheet .file-line')[1];
    expect(second?.querySelector('.tk-comment')?.textContent).toBe('   of a comment */');
  });

  it('draws everything plain again when colours are turned off', async () => {
    const { views, container } = mounted('inline');
    await vi.waitFor(() => expect(container.querySelector('tr.add .tk-type')).not.toBeNull());
    views.set('syntax', false);
    await vi.waitFor(() => expect(container.querySelector('[class^="tk-"]')).toBeNull());
    expect(container.querySelector('tr.add .text')?.textContent).toContain('int a = 2;');
  });
});
