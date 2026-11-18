import { describe, expect, it, vi } from 'vitest';
import hljs from 'highlight.js/lib/core';
import c from 'highlight.js/lib/languages/c';
import typescript from 'highlight.js/lib/languages/typescript';

import { grammarOf } from '../src/syntax/grammar';
import { tokensOf } from '../src/syntax/token';
import { segmentsOf } from '../src/syntax/segment';
import { SyntaxState } from '../src/state/syntax.svelte';
import { ViewsState } from '../src/state/views.svelte';

// The worker, run in this thread: the same library and the same reading of its answer.
const asked: string[] = [];
vi.mock('../src/syntax/highlighter', async () => {
  const { default: core } = await import('highlight.js/lib/core');
  const { default: cGrammar } = await import('highlight.js/lib/languages/c');
  const { tokensOf: read } = await import('../src/syntax/token');
  core.registerLanguage('c', cGrammar);
  return {
    highlight: async (grammar: string, text: string) => {
      asked.push(text);
      return read(core.highlight(text, { language: grammar, ignoreIllegals: true }).value);
    },
  };
});

hljs.registerLanguage('c', c);
hljs.registerLanguage('typescript', typescript);

function coloured(grammar: string, text: string) {
  return tokensOf(hljs.highlight(text, { language: grammar, ignoreIllegals: true }).value);
}

/** What each coloured stretch of a line reads, beside its colour. */
function read(text: string, tokens: { from: number; to: number; category: string }[]) {
  return tokens.map((t) => [text.slice(t.from, t.to), t.category]);
}

describe('choosing a grammar', () => {
  it('goes by the name, then by the extension, and draws anything else plain', () => {
    expect(grammarOf('kernel/sched/core.c')).toBe('c');
    expect(grammarOf('ui/src/App.TS')).toBe('typescript');
    expect(grammarOf('Makefile')).toBe('makefile');
    expect(grammarOf('drivers/net/Kbuild')).toBe('makefile');
    expect(grammarOf('Cargo.toml')).toBe('ini');
    expect(grammarOf('CMakeLists.txt')).toBe('cmake');
    // A hidden file's leading dot is its name, not an extension.
    expect(grammarOf('.bashrc')).toBeNull();
    expect(grammarOf('notes.txt')).toBeNull();
    expect(grammarOf('MAINTAINERS')).toBeNull();
  });
});

describe('reading highlight.js into lines', () => {
  it('numbers the columns of the code, not of the markup around it', () => {
    const lines = coloured('c', 'int a = 1 < 2 && "x";');
    expect(lines).toHaveLength(1);
    // `<` and `&&` arrive as entities and count as one column each.
    expect(read('int a = 1 < 2 && "x";', lines[0] ?? [])).toEqual([
      ['int', 'type'], ['1', 'constant'], ['2', 'constant'], ['"x"', 'string'],
    ]);
  });

  it('carries a comment across the lines it spans', () => {
    // The reason whole files are coloured rather than a hunk's few lines: the middle of a
    // block comment looks like code on its own.
    const text = '/* one\n   two */\nint x;';
    const lines = coloured('c', text);
    expect(lines.map((l, i) => read(text.split('\n')[i] ?? '', l))).toEqual([
      [['/* one', 'comment']],
      [['   two */', 'comment']],
      [['int', 'type']],
    ]);
  });

  it('leaves the code inside a template string uncoloured rather than calling it a string', () => {
    const text = 'const s = `a${b}c`;';
    expect(read(text, coloured('typescript', text)[0] ?? [])).toEqual([
      ['const', 'keyword'], ['`a', 'string'], ['c`', 'string'],
    ]);
  });

  it('answers a file with one list per line, the last line included', () => {
    expect(coloured('c', 'a;\n\nb;\n')).toHaveLength(4);
  });
});

describe('a line in pieces', () => {
  it('splits it where its colour or its mark changes, and nowhere else', () => {
    const text = 'return count;';
    const tokens = [{ from: 0, to: 6, category: 'keyword' as const }];
    const spans = [
      { text: 'return ', marked: false },
      { text: 'count', marked: true },
      { text: ';', marked: false },
    ];
    expect(segmentsOf(text, tokens, spans)).toEqual([
      { text: 'return', marked: false, category: 'keyword' },
      { text: ' ', marked: false, category: null },
      { text: 'count', marked: true, category: null },
      { text: ';', marked: false, category: null },
    ]);
  });

  it('marks across a token that a changed word cuts through', () => {
    const tokens = [{ from: 0, to: 6, category: 'string' as const }];
    const spans = [{ text: '"ab', marked: false }, { text: 'cd"', marked: true }, { text: ';', marked: false }];
    expect(segmentsOf('"abcd";', tokens, spans)).toEqual([
      { text: '"ab', marked: false, category: 'string' },
      { text: 'cd"', marked: true, category: 'string' },
      { text: ';', marked: false, category: null },
    ]);
  });

  it('ignores tokens that reach past the line, rather than drawing text that is not there', () => {
    expect(segmentsOf('ab', [{ from: 1, to: 9, category: 'comment' }], null)).toEqual([
      { text: 'a', marked: false, category: null },
      { text: 'b', marked: false, category: 'comment' },
    ]);
  });
});

describe('the colours the window keeps', () => {
  it('asks once for a file and draws it plain until the answer is back', async () => {
    asked.length = 0;
    const syntax = new SyntaxState(new ViewsState());
    const text = 'int a;\n';
    expect(syntax.tokens('c', text)).toBeNull();
    syntax.want('c', text);
    syntax.want('c', text);
    await vi.waitFor(() => expect(syntax.tokens('c', text)).not.toBeNull());
    expect(asked).toEqual([text]);
    expect(syntax.tokens('c', text)?.[0]).toEqual([{ from: 0, to: 3, category: 'type' }]);
    // The same text under another grammar is another question.
    expect(syntax.tokens('rust', text)).toBeNull();
  });

  it('leaves a file too large to be read for its colours plain, without asking', () => {
    asked.length = 0;
    const syntax = new SyntaxState(new ViewsState());
    syntax.want('c', 'x;\n'.repeat(20_001));
    syntax.want('c', 'x'.repeat(1_000_001));
    expect(asked).toEqual([]);
  });

  it('draws everything plain with colours turned off', async () => {
    const views = new ViewsState();
    const syntax = new SyntaxState(views);
    syntax.want('c', 'int b;\n');
    await vi.waitFor(() => expect(syntax.tokens('c', 'int b;\n')).not.toBeNull());
    views.set('syntax', false);
    expect(syntax.tokens('c', 'int b;\n')).toBeNull();
    asked.length = 0;
    syntax.want('c', 'int c;\n');
    expect(asked).toEqual([]);
  });
});
