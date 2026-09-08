import { describe, expect, it } from 'vitest';

import { markedPair, wordMarks, type Span } from '../src/diff/word';
import type { Hunk, Line, LineKind } from '../src/ipc/types';

/** The marked parts of one side, which is what a reader's eye is drawn to. */
function marked(spans: readonly Span[]): string[] {
  return spans.filter((s) => s.marked).map((s) => s.text);
}

/** Every span joined back together, which must be the line it came from. */
function whole(spans: readonly Span[]): string {
  return spans.map((s) => s.text).join('');
}

function line(kind: LineKind, text: string): Line {
  return { kind, text, oldNo: null, newNo: null, noNewline: false };
}

function hunk(...lines: Line[]): Hunk {
  return { header: '@@', oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines };
}

describe('what changed inside a line', () => {
  it('marks the one word that differs and leaves the rest alone', () => {
    const pair = markedPair('let total = count + 1;', 'let total = amount + 1;');
    expect(marked(pair?.left ?? [])).toEqual(['count']);
    expect(marked(pair?.right ?? [])).toEqual(['amount']);
  });

  it('gives back every character of the line it was handed', () => {
    // The spans are what gets rendered, so anything dropped here is text missing from the diff.
    const before = '  return Ok(self.head()?);';
    const after = '  return Ok(self.head(name)?);';
    const pair = markedPair(before, after);
    expect(whole(pair?.left ?? [])).toBe(before);
    expect(whole(pair?.right ?? [])).toBe(after);
  });

  it('marks two separate edits separately rather than everything between them', () => {
    // A common prefix and suffix alone would mark the whole middle, which on a line with an
    // edit near each end is the whole line.
    const pair = markedPair('draw(first, middle, last)', 'draw(one, middle, two)');
    expect(marked(pair?.left ?? [])).toEqual(['first', 'last']);
    expect(marked(pair?.right ?? [])).toEqual(['one', 'two']);
  });

  it('marks a word, not the letters inside it', () => {
    // Character by character these two share `sort_`, `_by` and four letters in between, and
    // the mark comes out as confetti across a name that simply changed.
    const pair = markedPair('rows.sort_by(key);', 'rows.sort_unstable_by(key);');
    expect(marked(pair?.left ?? [])).toEqual(['sort_by']);
    expect(marked(pair?.right ?? [])).toEqual(['sort_unstable_by']);
  });

  it('marks nothing on the side a word was only added to', () => {
    const pair = markedPair('call(a)', 'call(a, b)');
    expect(marked(pair?.left ?? [])).toEqual([]);
    expect(marked(pair?.right ?? [])).toEqual([', b']);
  });

  it('marks the change in indentation, which is the only thing that happened', () => {
    const pair = markedPair('  guard();', '      guard();');
    expect(marked(pair?.left ?? [])).toEqual(['  ']);
    expect(marked(pair?.right ?? [])).toEqual(['      ']);
  });

  it('marks what was added to the end of a line, however much that is', () => {
    // Seventy per cent of the new line is new, and every word of the old one is still there.
    // A guard on how much changed threw this away; a guard on how much survived keeps it.
    const pair = markedPair('line two', 'line two, rewritten by hand');
    expect(marked(pair?.left ?? [])).toEqual([]);
    expect(marked(pair?.right ?? [])).toEqual([', rewritten by hand']);
  });

  it('says nothing when the two lines share only their indentation', () => {
    // Blank space is not evidence: leading spaces are what every line in a file has.
    expect(markedPair('    alpha', '    beta_gamma')).toBeNull();
  });

  it('says nothing when what survived is too short to point at', () => {
    expect(markedPair('x', 'x' + ' plus a great deal more that is entirely new')).toBeNull();
  });

  it('says nothing at all where the line was rewritten', () => {
    // Marking nine tenths of a line only repeats what the line's own colour already said.
    expect(markedPair('let total = count + 1;', 'emit(&mut out, "done")?;')).toBeNull();
  });

  it('says nothing for two lines that are the same', () => {
    expect(markedPair('same()', 'same()')).toBeNull();
  });

  it('marks a long line without building a table it cannot afford', () => {
    // A minified bundle is one line of a hundred thousand characters. The pair budget stops the
    // token table there; what matters is that it answers at all, and quickly.
    const before = `x=${'a,'.repeat(20_000)}end`;
    const after = `x=${'a,'.repeat(20_000)}fin`;
    const began = Date.now();
    const pair = markedPair(before, after);
    expect(Date.now() - began, 'answers without stalling the panel').toBeLessThan(2_000);
    expect(marked(pair?.left ?? [])).toEqual(['end']);
    expect(marked(pair?.right ?? [])).toEqual(['fin']);
  });
});

describe('the marks in a file', () => {
  it('pairs each removed line with the added one opposite it', () => {
    const before = [line('remove', 'one(a)'), line('remove', 'two(a)')];
    const after = [line('add', 'one(b)'), line('add', 'two(b)')];
    const marks = wordMarks([hunk(line('context', 'head'), ...before, ...after)]);

    expect(marked(marks.get(before[0] as Line) ?? [])).toEqual(['a']);
    expect(marked(marks.get(after[1] as Line) ?? [])).toEqual(['b']);
  });

  it('leaves a run that is an insertion alone', () => {
    // Nothing was replaced, so there is nothing to compare the new lines against, and every
    // one of them would come out marked end to end.
    const added = line('add', 'let extra = 1;');
    const marks = wordMarks([hunk(line('context', 'head'), added, line('context', 'tail'))]);
    expect(marks.has(added)).toBe(false);
  });

  it('keeps runs apart, so a line is only compared with its own', () => {
    const first = line('remove', 'alpha(1)');
    const second = line('remove', 'beta(2)');
    const marks = wordMarks([
      hunk(first, line('add', 'alpha(9)'), line('context', 'between'), second, line('add', 'beta(8)')),
    ]);
    expect(marked(marks.get(first) ?? [])).toEqual(['1']);
    expect(marked(marks.get(second) ?? [])).toEqual(['2']);
  });

  it('marks a file where every line changed without holding the panel up', () => {
    // A generated file re-emitted is thousands of replaced lines, and the table is quadratic
    // in the length of a line. This is built once, before the panel draws anything.
    const removed = Array.from({ length: 2_000 }, (_, i) =>
      line('remove', `  pub const ENTRY_${i}: u32 = ${i} * 3 + 1;`),
    );
    const added = Array.from({ length: 2_000 }, (_, i) =>
      line('add', `  pub const ENTRY_${i}: u32 = ${i} * 4 + 1;`),
    );

    const began = Date.now();
    const marks = wordMarks([hunk(...removed, ...added)]);
    expect(Date.now() - began, 'the file budget holds the work down').toBeLessThan(1_500);
    // The budget runs out partway through, and what is past it is left unmarked rather than
    // the panel being late. What is before it is marked.
    expect(marks.size, 'the lines it reached are marked').toBeGreaterThan(200);
    expect(marked(marks.get(removed[0] as Line) ?? [])).toEqual(['3']);
  });

  it('leaves a line whose change is the whole line unmarked', () => {
    const from = line('remove', 'let total = count + 1;');
    const to = line('add', 'emit(&mut out, "done")?;');
    const marks = wordMarks([hunk(from, to)]);
    expect(marks.has(from)).toBe(false);
    expect(marks.has(to)).toBe(false);
  });
});
