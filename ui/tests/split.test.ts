import { describe, expect, it } from 'vitest';

import { splitRows } from '../src/diff/split';
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
