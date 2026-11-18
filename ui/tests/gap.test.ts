import { describe, expect, it } from 'vitest';

import { gapsOf } from '../src/diff/gap';
import type { Hunk } from '../src/ipc/types';

/** A hunk by its header's four numbers; its lines do not enter into where the gaps are. */
function hunk(oldStart: number, oldLines: number, newStart: number, newLines: number): Hunk {
  return { header: '', oldStart, oldLines, newStart, newLines, lines: [] };
}

describe('the gaps between hunks', () => {
  it('finds the stretch before the first hunk, between two and after the last', () => {
    // A three-line hunk at 10 and another at 40 in a fifty-line file.
    expect(gapsOf([hunk(10, 3, 10, 3), hunk(40, 3, 40, 3)], 50)).toEqual([
      { below: 0, first: 1, last: 9, shift: 0 },
      { below: 1, first: 13, last: 39, shift: 0 },
      { below: 2, first: 43, last: 50, shift: 0 },
    ]);
  });

  it('numbers the old side across a gap from what the hunk before it added or took away', () => {
    // Two lines added at 10, then three removed after new line 30.
    expect(gapsOf([hunk(9, 0, 10, 2), hunk(29, 3, 30, 0)], 40)).toEqual([
      { below: 0, first: 1, last: 9, shift: 0 },
      { below: 1, first: 12, last: 30, shift: -2 },
      { below: 2, first: 31, last: 40, shift: 1 },
    ]);
  });

  it('leaves out a gap with nothing in it', () => {
    expect(gapsOf([hunk(1, 3, 1, 3), hunk(4, 2, 4, 2)], 5)).toEqual([]);
  });

  it('has no gaps for a file that is all one hunk', () => {
    expect(gapsOf([hunk(0, 0, 1, 7)], 7)).toEqual([]);
  });
});
