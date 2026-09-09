import { commitDetail, compareCommits } from '../ipc/commands';

import { rowOfOid, type Frame } from '../graph/frame';
import type { ChangedFile, CommitDetail } from '../ipc/types';
import { messageOf } from '../ipc/error';

/** One end of a comparison: which row was picked, and the commit on it. */
export interface Picked {
  row: number;
  oid: string;
}

/**
 * The selected commit and its details, or the pair being compared.
 *
 * Details are fetched per selection rather than held for every row: the file list of a large
 * merge is thousands of entries, and the graph has a million rows.
 */
export class SelectionState {
  row = $state<number | null>(null);
  detail = $state<CommitDetail | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  /**
   * The two commits being compared, oldest first, or null when one is selected.
   *
   * Oldest first whichever order they were picked in: a comparison read the other way calls
   * every file the newer commit added a deletion, which is true of the direction and useless
   * as an answer to "what changed between these two".
   */
  pair = $state<{ from: Picked; to: Picked } | null>(null);
  /** What differs between the pair, once it has been read. */
  compared = $state<ChangedFile[]>([]);

  #token = 0;
  /** The commit the pair would be built from, kept so the second pick has a first. */
  #anchor: Picked | null = null;

  clear(): void {
    this.#token += 1;
    this.row = null;
    this.detail = null;
    this.error = null;
    this.pair = null;
    this.compared = [];
    this.#anchor = null;
  }

  async select(path: string, row: number, oid: string): Promise<void> {
    this.#anchor = { row, oid };
    this.pair = null;
    this.compared = [];
    this.row = row;
    this.loading = true;
    this.error = null;

    // Clicking down the graph starts several reads; only the newest may write the result,
    // otherwise a slow earlier one lands last and shows the wrong commit.
    const token = ++this.#token;
    try {
      const detail = await commitDetail(path, oid);
      if (token === this.#token) this.detail = detail;
    } catch (e) {
      if (token === this.#token) {
        this.error = messageOf(e);
        this.detail = null;
      }
    } finally {
      if (token === this.#token) this.loading = false;
    }
  }

  /**
   * Compares the commit already selected with another.
   *
   * Falls back to selecting it outright when there is nothing to compare against, and when the
   * same row is picked twice — comparing a commit with itself is an empty answer nobody asked
   * for.
   */
  async compare(path: string, row: number, oid: string): Promise<void> {
    const anchor = this.#anchor;
    if (anchor === null || anchor.row === row) {
      await this.select(path, row, oid);
      return;
    }

    // Rows count back through history, so the larger row number is the older commit.
    const picked = { row, oid };
    const [from, to] = anchor.row > row ? [anchor, picked] : [picked, anchor];
    this.pair = { from, to };
    this.detail = null;
    this.compared = [];
    this.loading = true;
    this.error = null;

    const token = ++this.#token;
    try {
      const files = await compareCommits(path, from.oid, to.oid);
      if (token === this.#token) this.compared = files;
    } catch (e) {
      if (token === this.#token) {
        this.error = messageOf(e);
        this.compared = [];
      }
    } finally {
      if (token === this.#token) this.loading = false;
    }
  }

  /**
   * Puts the selection back on the commit it was made on, after the rows were renumbered.
   *
   * Rows are the walk's numbering, not a commit's identity. Anything that writes a commit —
   * committing, amending, reverting, cherry-picking, rebasing, resetting — shifts every row
   * under it, so a selection held as a row number silently comes to mark a different commit
   * from the one the panel beside it is describing.
   *
   * Only the loaded window is searched. It is what is in memory, and a commit outside it is
   * either gone or a long way from where the reader is looking; either way, letting the
   * selection go beats moving it to a row that merely has the right number.
   */
  reanchor(frame: Frame | null): void {
    if (frame === null) return;
    const pair = this.pair;
    if (pair !== null) {
      const from = rowOfOid(frame, pair.from.oid);
      const to = rowOfOid(frame, pair.to.oid);
      if (from === null || to === null) {
        this.clear();
        return;
      }
      if (from === pair.from.row && to === pair.to.row) return;
      this.pair = { from: { ...pair.from, row: from }, to: { ...pair.to, row: to } };
      if (this.#anchor !== null) {
        this.#anchor = { ...this.#anchor, row: this.#anchor.oid === pair.from.oid ? from : to };
      }
      return;
    }

    const anchor = this.#anchor;
    if (anchor === null || this.row === null) return;
    const found = rowOfOid(frame, anchor.oid);
    if (found === null) {
      this.clear();
      return;
    }
    if (found === this.row) return;
    this.row = found;
    this.#anchor = { ...anchor, row: found };
  }

  /** True for a row that is one end of the comparison, so the list can mark both. */
  marks(row: number): boolean {
    const pair = this.pair;
    if (pair === null) return this.row === row;
    return pair.from.row === row || pair.to.row === row;
  }
}
