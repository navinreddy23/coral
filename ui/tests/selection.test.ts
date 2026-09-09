import { describe, expect, it, vi } from 'vitest';

vi.mock('../src/ipc/commands', () => ({
  commitDetail: vi.fn(async () => ({ oid: 'x', files: [] })),
  compareCommits: vi.fn(async () => []),
}));

import { SelectionState } from '../src/state/selection.svelte';
import type { Frame } from '../src/graph/frame';

function frameOf(startRow: number, oids: string[]): Frame {
  const rowCount = oids.length;
  const bytes = new Uint8Array(rowCount * 20);
  oids.forEach((oid, row) => {
    for (let i = 0; i < 20; i += 1) {
      bytes[row * 20 + i] = Number.parseInt(oid.slice(i * 2, i * 2 + 2), 16);
    }
  });
  return {
    startRow,
    rowCount,
    totalRows: startRow + rowCount,
    flags: 0,
    hashLen: 20,
    lanes: new Uint16Array(rowCount),
    rowFlags: new Uint8Array(rowCount),
    times: new Float64Array(rowCount),
    parentStart: new Uint32Array(rowCount + 1),
    parentLanes: new Uint16Array(0),
    oids: bytes,
    open: new Uint32Array(rowCount),
  };
}

const repo = '/home/dev/journal';
const tea = 'a'.repeat(40);
const coffee = 'b'.repeat(40);
const water = 'c'.repeat(40);
const revert = 'd'.repeat(40);
const other = 'e'.repeat(40);

describe('a selection that outlives the rows it was made on', () => {
  /**
   * Reverting, committing, amending and rebasing all put a commit above the one selected, and
   * every row below it is renumbered. Held as a row number the highlight stayed put and came
   * to mark the commit that had moved into that row, while the panel beside it went on
   * describing the commit that was picked. Two answers to "which commit is this" on one screen.
   */
  it('follows the commit down when something is committed above it', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 1, coffee);
    expect(selection.marks(1)).toBe(true);

    selection.reanchor(frameOf(0, [revert, tea, coffee, water]));

    expect(selection.row).toBe(2);
    expect(selection.marks(2)).toBe(true);
    expect(selection.marks(1)).toBe(false);
  });

  it('leaves a selection alone when the rows did not move', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 1, coffee);
    selection.reanchor(frameOf(0, [tea, coffee, water]));
    expect(selection.row).toBe(1);
    expect(selection.detail).not.toBeNull();
  });

  it("counts from the frame's own first row, not from zero", async () => {
    const selection = new SelectionState();
    await selection.select(repo, 4098, coffee);
    selection.reanchor(frameOf(4096, [tea, coffee, water]));
    expect(selection.row).toBe(4097);
  });

  it('lets the selection go when the commit is no longer in the window', async () => {
    // Rebasing away the commit that was selected, or scrolling a million rows from it. Marking
    // whichever row now carries that number is the thing being fixed, so nothing is marked.
    const selection = new SelectionState();
    await selection.select(repo, 1, coffee);
    selection.reanchor(frameOf(0, [tea, water]));
    expect(selection.row).toBeNull();
    expect(selection.detail).toBeNull();
    expect(selection.marks(1)).toBe(false);
  });

  it('does nothing at all when there is no selection, or no frame', async () => {
    const selection = new SelectionState();
    selection.reanchor(frameOf(0, [tea]));
    expect(selection.row).toBeNull();

    await selection.select(repo, 1, coffee);
    selection.reanchor(null);
    expect(selection.row).toBe(1);
  });

  it('moves both ends of a comparison, and drops it when either end has gone', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 0, tea);
    await selection.compare(repo, 2, water);
    expect(selection.pair).toEqual({ from: { row: 2, oid: water }, to: { row: 0, oid: tea } });

    selection.reanchor(frameOf(0, [revert, tea, coffee, water]));
    expect(selection.pair).toEqual({ from: { row: 3, oid: water }, to: { row: 1, oid: tea } });
    expect(selection.marks(3)).toBe(true);
    expect(selection.marks(1)).toBe(true);

    selection.reanchor(frameOf(0, [revert, coffee, water, other]));
    expect(selection.pair).toBeNull();
    expect(selection.marks(2)).toBe(false);
  });

  /**
   * A frame is a window, not the whole graph. Scrolling away from a selected commit on a
   * repository worth paging loads one that does not hold it, and reading that as "the commit
   * is gone" threw the selection away for scrolling.
   */
  it('says nothing about a selection the frame does not reach', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 4097, coffee);

    // A window somewhere else entirely.
    selection.reanchor(frameOf(0, [tea, water]));

    expect(selection.row).toBe(4097);
    expect(selection.detail).not.toBeNull();
  });

  it('still lets it go when the frame does reach and the commit is not there', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 1, coffee);
    selection.reanchor(frameOf(0, [tea, water, revert]));
    expect(selection.row).toBeNull();
  });
});
