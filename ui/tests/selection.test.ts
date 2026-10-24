import { describe, expect, it, vi } from 'vitest';

/** Signature checks the test answers by hand, by revision, to play them out of order. */
const checks = vi.hoisted(() => new Map<string, (v: unknown) => void>());

vi.mock('../src/ipc/commands', () => ({
  commitDetail: vi.fn(async () => ({ oid: 'x', files: [] })),
  commitVerification: vi.fn((_path: string, rev: string) => new Promise((done) => checks.set(rev, done))),
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

describe('the signature beside the detail', () => {
  it('shows the detail without waiting, and drops a verdict for a commit no longer selected', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 0, tea);
    // The detail is in while the check is still out.
    expect(selection.detail).not.toBeNull();
    expect(selection.verification).toBeNull();

    await selection.select(repo, 1, coffee);
    checks.get(tea)?.({ kind: 'bad', signer: '' });
    await Promise.resolve();
    expect(selection.verification).toBeNull();

    checks.get(coffee)?.({ kind: 'unsigned' });
    await Promise.resolve();
    expect(selection.verification).toEqual({ kind: 'unsigned' });
  });

  it('does not ask about a stash, which is never signed', async () => {
    checks.clear();
    const selection = new SelectionState();
    await selection.select(repo, 0, water, 'stash');
    expect(checks.has(water)).toBe(false);
  });
});

describe('picking several commits', () => {
  it('compares two, makes a set of three, and goes back to comparing when one is dropped', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 0, tea);
    await selection.toggle(repo, 2, water);
    expect(selection.pair?.from.oid).toBe(water);
    expect(selection.many).toEqual([]);

    await selection.toggle(repo, 1, coffee);
    expect(selection.pair).toBeNull();
    expect(selection.many.map((p) => p.oid)).toEqual([tea, coffee, water]);
    expect([0, 1, 2].every((row) => selection.marks(row))).toBe(true);
    expect(selection.marks(3)).toBe(false);

    await selection.toggle(repo, 1, coffee);
    expect(selection.many).toEqual([]);
    expect(selection.pair?.to.oid).toBe(tea);
  });

  it('keeps the anchor a run is measured from', async () => {
    const selection = new SelectionState();
    await selection.select(repo, 1, coffee);
    await selection.pickAll(repo, [
      { row: 1, oid: coffee },
      { row: 2, oid: water },
      { row: 3, oid: revert },
    ]);
    expect(selection.anchorRow).toBe(1);
    expect(selection.picked.map((p) => p.row)).toEqual([1, 2, 3]);
  });

  it('leaves a set that has not moved exactly as it was', () => {
    // reanchor runs from an effect that reads the set; a fresh array every time re-ran that
    // effect without end, and the window stopped answering clicks.
    const selection = new SelectionState();
    void selection.pickAll(repo, [
      { row: 0, oid: tea },
      { row: 1, oid: coffee },
      { row: 2, oid: water },
    ]);
    const before = selection.many;
    selection.reanchor(frameOf(0, [tea, coffee, water]));
    expect(selection.many).toBe(before);
  });

  it('follows a set to its new rows, and lets it go when a commit in it is gone', () => {
    const selection = new SelectionState();
    void selection.pickAll(repo, [
      { row: 0, oid: tea },
      { row: 1, oid: coffee },
      { row: 2, oid: water },
    ]);
    selection.reanchor(frameOf(0, [other, tea, coffee, water]));
    expect(selection.many.map((p) => p.row)).toEqual([1, 2, 3]);
    selection.reanchor(frameOf(0, [other, tea, water, revert]));
    expect(selection.many).toEqual([]);
  });
});
