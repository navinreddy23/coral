import type { Hunk } from '../ipc/types';

/**
 * A stretch of unchanged lines a unified diff leaves out: before the first hunk, between two,
 * or after the last.
 *
 * Numbered on the new side, `first` to `last`. Nothing in a gap changed, so the old side's
 * number is the new one moved by `shift` all the way across it. `below` is the hunk the gap
 * sits above, or the number of hunks for the one after the last.
 */
export interface Gap {
  below: number;
  first: number;
  last: number;
  shift: number;
}

/**
 * The gaps around `hunks` in a file whose new side has `lines` lines.
 *
 * A side with no lines in a hunk names the line it comes after, which is git's convention for
 * an empty range, so the hunk sits one further on than its start says.
 */
export function gapsOf(hunks: readonly Hunk[], lines: number): Gap[] {
  const out: Gap[] = [];
  let next = 1;
  let shift = 0;
  hunks.forEach((h, i) => {
    const start = h.newLines === 0 ? h.newStart + 1 : h.newStart;
    if (start > next) out.push({ below: i, first: next, last: start - 1, shift });
    next = start + h.newLines;
    const oldNext = (h.oldLines === 0 ? h.oldStart + 1 : h.oldStart) + h.oldLines;
    shift = oldNext - next;
  });
  if (lines >= next) out.push({ below: hunks.length, first: next, last: lines, shift });
  return out;
}
