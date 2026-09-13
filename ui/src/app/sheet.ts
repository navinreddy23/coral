/**
 * The rows a merge tool's sheets draw: one side's version of the file, or the result.
 *
 * A module of its own because the sheet component and the tool that lays three of them out
 * both need it, and a component cannot be imported from one that imports it back.
 */
import type { Block } from '../ipc/types';
import type { Pick, Side } from '../state/merge.svelte';
import { markersFor, sideOf } from '../state/merge.svelte';

/** One line of a file as a pane draws it. */
export interface Row {
  /** Position in the pane, which is what keys the list. */
  at: number;
  /** Line number in this version of the file, or null for a placeholder. */
  no: number | null;
  text: string;
  /** Which conflict the line belongs to, or null for a line both sides agree on. */
  conflict: number | null;
  /** Position within that side of the conflict, which is what a pick names. */
  line: number | null;
  /** True for the first line of a conflict, which is what the stepper scrolls to. */
  first: boolean;
}

/** The file as one side has it: every agreed line, plus that side of every conflict. */
export function sideRows(blocks: readonly Block[], side: Side): Row[] {
  const out: Row[] = [];
  let conflict = 0;
  for (const block of blocks) {
    if (block.kind === 'common') {
      for (const text of block.lines) push(out, text, null, null, false);
      continue;
    }
    const lines = sideOf(block, side);
    lines.forEach((text, i) => push(out, text, conflict, i, i === 0));
    // A side that adds nothing here still needs a row, or the conflict has no place in
    // this pane at all and the stepper has nothing to scroll to.
    if (lines.length === 0) push(out, '(nothing on this side)', conflict, null, true, false);
    conflict += 1;
  }
  return out;
}

/**
 * The file as it will be written, with each line tied back to the conflict it came from.
 *
 * A region nobody has answered draws the markers it would be written as, so the pane shows the
 * file rather than a guess at it.
 */
export function outputRows(
  blocks: readonly Block[],
  choices: Record<number, Pick>,
  labels: { ours: string; theirs: string } = { ours: 'ours', theirs: 'theirs' },
): Row[] {
  const out: Row[] = [];
  let conflict = 0;
  for (const block of blocks) {
    if (block.kind === 'common') {
      for (const text of block.lines) push(out, text, null, null, false);
      continue;
    }
    const pick = choices[conflict];
    if (pick === undefined) {
      // No `line`, because a marker is not a line of either side and cannot be picked.
      markersFor(block, labels).forEach((text, i) => push(out, text, conflict, null, i === 0));
      conflict += 1;
      continue;
    }
    const lines = pick.map((t) => sideOf(block, t.side)[t.line]).filter((l) => l !== undefined);
    lines.forEach((text, i) => push(out, text, conflict, i, i === 0));
    if (lines.length === 0) push(out, '(nothing taken)', conflict, null, true, false);
    conflict += 1;
  }
  return out;
}

/** Appends a row, numbering it. A placeholder stands in for a line and takes no number. */
function push(
  out: Row[],
  text: string,
  conflict: number | null,
  line: number | null,
  first: boolean,
  real = true,
) {
  const previous = out.at(-1);
  const before = previous?.no ?? 0;
  out.push({ at: out.length, no: real ? before + 1 : null, text, conflict, line, first });
}
