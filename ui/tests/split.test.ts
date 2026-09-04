import { describe, expect, it } from 'vitest';

import { marksOf, splitRows, windowAround, type SplitRow } from '../src/diff/split';
import type { Hunk, Line, LineKind } from '../src/ipc/types';

function line(kind: LineKind, text: string): Line {
  return { kind, text, oldNo: null, newNo: null, noNewline: false };
}

function hunk(...lines: Line[]): Hunk {
  return { header: '@@', oldStart: 1, oldLines: 0, newStart: 1, newLines: 0, lines };
}

/** Each row as `left|right`, with a dash for a blank side. */
function render(rows: ReturnType<typeof splitRows>): string[] {
  return rows.map((r) => `${r.left?.text ?? '-'}|${r.right?.text ?? '-'}`);
}

describe('splitRows', () => {
  it('puts context on both sides', () => {
    expect(render(splitRows(hunk(line('context', 'a'), line('context', 'b'))))).toEqual([
      'a|a',
      'b|b',
    ]);
  });

  it('pairs a replaced line with its replacement', () => {
    const rows = splitRows(hunk(line('remove', 'old'), line('add', 'new')));
    expect(render(rows)).toEqual(['old|new']);
  });

  it('pairs a run positionally and leaves the shorter side blank', () => {
    const rows = splitRows(
      hunk(line('remove', 'r1'), line('remove', 'r2'), line('add', 'a1')),
    );
    expect(render(rows)).toEqual(['r1|a1', 'r2|-']);
  });

  it('shows a pure addition with nothing opposite it', () => {
    const rows = splitRows(hunk(line('context', 'c'), line('add', 'a1'), line('add', 'a2')));
    expect(render(rows)).toEqual(['c|c', '-|a1', '-|a2']);
  });

  it('keeps runs separate across an intervening context line', () => {
    const rows = splitRows(
      hunk(
        line('remove', 'r1'),
        line('add', 'a1'),
        line('context', 'c'),
        line('remove', 'r2'),
        line('add', 'a2'),
      ),
    );
    expect(render(rows)).toEqual(['r1|a1', 'c|c', 'r2|a2']);
  });

  it('pairs additions that git listed before the removals they replace', () => {
    // A hunk can open with additions; nothing may be lost when it does.
    const rows = splitRows(hunk(line('add', 'a1'), line('remove', 'r1')));
    expect(render(rows)).toEqual(['r1|a1']);
  });

  it('has no rows for an empty hunk', () => {
    expect(splitRows(hunk())).toEqual([]);
  });
});

describe('windowing a whole-file view', () => {
  const context = (n: number): SplitRow => ({
    left: { kind: 'context', text: `l${n}`, oldNo: n, newNo: n, noNewline: false },
    right: { kind: 'context', text: `l${n}`, oldNo: n, newNo: n, noNewline: false },
  });
  const added = (n: number): SplitRow => ({
    left: null,
    right: { kind: 'add', text: `new ${n}`, oldNo: null, newNo: n, noNewline: false },
  });

  it('leaves a file that fits alone', () => {
    const rows = [context(1), added(2), context(3)];
    const got = windowAround(rows, 10);
    expect(got.from).toBe(0);
    expect(got.rows).toEqual(rows);
  });

  it('keeps the change on screen when the file is longer than the budget', () => {
    // The change is far past the budget, so a window taken from the top would show a file with
    // no change in it.
    const rows = [...Array.from({ length: 900 }, (_, i) => context(i + 1)), added(901)];
    const got = windowAround(rows, 100);

    expect(got.rows).toHaveLength(100);
    expect(got.rows.some((r) => r.right?.kind === 'add')).toBe(true);
    // A quarter of the budget of context above it, so it does not sit on the first line.
    expect(got.from).toBe(rows.length - 100);
  });

  it('puts context above the change rather than starting on it', () => {
    const rows = [
      ...Array.from({ length: 400 }, (_, i) => context(i + 1)),
      added(401),
      ...Array.from({ length: 400 }, (_, i) => context(i + 402)),
    ];
    const got = windowAround(rows, 100);

    expect(got.from).toBe(400 - 25);
    expect(got.rows[25]?.right?.kind).toBe('add');
  });

  it('takes the top when nothing changed', () => {
    const rows = Array.from({ length: 300 }, (_, i) => context(i + 1));
    const got = windowAround(rows, 50);
    expect(got.from).toBe(0);
    expect(got.rows).toHaveLength(50);
  });
});

describe('marking where the changes are', () => {
  const context = (n: number): SplitRow => ({
    left: { kind: 'context', text: `l${n}`, oldNo: n, newNo: n, noNewline: false },
    right: { kind: 'context', text: `l${n}`, oldNo: n, newNo: n, noNewline: false },
  });
  const added = (n: number): SplitRow => ({
    left: null,
    right: { kind: 'add', text: `new ${n}`, oldNo: null, newNo: n, noNewline: false },
  });
  const removed = (n: number): SplitRow => ({
    left: { kind: 'remove', text: `old ${n}`, oldNo: n, newNo: null, noNewline: false },
    right: null,
  });

  it('is one mark per run, not one per line', () => {
    const rows = [context(1), added(2), added(3), added(4), context(5)];
    const marks = marksOf(rows);
    expect(marks).toHaveLength(1);
    expect(marks[0]?.at).toBeCloseTo(1 / 5);
    expect(marks[0]?.size).toBeCloseTo(3 / 5);
    expect(marks[0]?.kind).toBe('add');
  });

  it('tells a replacement from an insertion', () => {
    expect(marksOf([removed(1), added(1)])[0]?.kind).toBe('both');
    expect(marksOf([context(1), removed(2)])[0]?.kind).toBe('remove');
  });

  it('finds every run down a long file', () => {
    const rows = [
      ...Array.from({ length: 50 }, (_, i) => context(i + 1)),
      added(51),
      ...Array.from({ length: 50 }, (_, i) => context(i + 52)),
      removed(102),
    ];
    const marks = marksOf(rows);
    expect(marks.map((m) => m.kind)).toEqual(['add', 'remove']);
    expect(marks[0]?.at).toBeCloseTo(50 / 102);
  });

  it('marks nothing in a file that did not change', () => {
    expect(marksOf([context(1), context(2)])).toEqual([]);
    expect(marksOf([])).toEqual([]);
  });
});
