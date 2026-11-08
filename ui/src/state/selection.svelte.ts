import {
  changedSince,
  commitDetail,
  commitNote,
  commitVerification,
  compareCommits,
  mergeBase,
  type Listing,
} from '../ipc/commands';

import { covers, rowOfOid, type Frame } from '../graph/frame';
import type { ChangedFile, CommitDetail, Verification } from '../ipc/types';
import { messageOf } from '../ipc/error';

/** One end of a comparison: which row was picked, and the commit on it. */
export interface Picked {
  row: number;
  oid: string;
}

/** One end of a comparison asked for by name: the commit, and what the user called it. */
export interface Named {
  oid: string;
  name: string;
}

/**
 * A comparison asked for from a menu rather than made by picking two rows.
 *
 * Held by revision, not by row: a branch compared with another need be nowhere in the rows
 * loaded, and the working tree is no row at all. `base` is where the two parted, once asked to
 * measure from there: what `to` gained since, rather than everything between the tips.
 */
export type Against =
  | { kind: 'worktree'; from: Named }
  | { kind: 'refs'; from: Named; to: Named; base: string | null };

/**
 * The selected commit and its details, or the pair being compared.
 *
 * Details are fetched per selection rather than held for every row: the file list of a large
 * merge is thousands of entries, and the graph has a million rows.
 */
export class SelectionState {
  row = $state<number | null>(null);
  detail = $state<CommitDetail | null>(null);
  /**
   * What git makes of the selected commit's signature, once it has said.
   *
   * Read beside the detail rather than with it: checking runs the signing program, which can
   * take seconds, and the message and files should not wait on it.
   */
  verification = $state<Verification | null>(null);
  /** The selected commit's note, read beside the detail as the signature is. */
  note = $state<string | null>(null);
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
  /** What differs between the pair, or between the ends of {@link against}, once read. */
  compared = $state<ChangedFile[]>([]);

  /** A comparison asked for from a menu; null otherwise. Never set alongside {@link pair}. */
  against = $state<Against | null>(null);

  /**
   * Three or more commits picked together, newest first; empty otherwise.
   *
   * Two picked are a comparison, which is what picking a second commit has always meant; past
   * two there is no one diff to show, only a set to cherry-pick or revert.
   */
  many = $state<Picked[]>([]);

  #token = 0;
  /** The commit the pair would be built from, kept so the second pick has a first. */
  #anchor: Picked | null = null;

  clear(): void {
    this.#token += 1;
    this.row = null;
    this.detail = null;
    this.verification = null;
    this.note = null;
    this.error = null;
    this.pair = null;
    this.compared = [];
    this.many = [];
    this.against = null;
    this.#anchor = null;
  }

  /** Reads the selected commit's note again, after it was edited. */
  async reloadNote(path: string): Promise<void> {
    const oid = this.detail?.commit.oid;
    if (oid !== undefined) await this.#readNote(path, oid, this.#token);
  }

  async #readNote(path: string, oid: string, token: number): Promise<void> {
    try {
      const note = await commitNote(path, oid);
      if (token === this.#token) this.note = note ?? null;
    } catch {
      // Nothing to show is the honest answer to a note that could not be read.
    }
  }

  /** Every commit picked, newest first, whatever the count. */
  get picked(): Picked[] {
    if (this.many.length > 0) return this.many;
    if (this.pair !== null) return [this.pair.to, this.pair.from];
    return this.#anchor !== null && this.row !== null ? [this.#anchor] : [];
  }

  /** The commit a run picked with Shift starts from: the last one picked on its own. */
  get anchorRow(): number | null {
    return this.#anchor?.row ?? null;
  }

  /** Adds a commit to the picked set, or takes it out if it is already there. */
  async toggle(path: string, row: number, oid: string): Promise<void> {
    const now = this.picked;
    const next = now.some((p) => p.row === row)
      ? now.filter((p) => p.row !== row)
      : [...now, { row, oid }];
    await this.pickAll(path, next);
  }

  /**
   * Picks exactly these commits.
   *
   * One is a selection and two a comparison, as before; three or more are a set. The anchor
   * is left where it was, so a second Shift-click measures from the same commit as the first.
   */
  async pickAll(path: string, picks: Picked[]): Promise<void> {
    const sorted = [...picks].sort((a, b) => a.row - b.row);
    const [first, second] = sorted;
    if (first === undefined) {
      this.clear();
      return;
    }
    const anchor = this.#anchor;
    if (second === undefined) {
      await this.select(path, first.row, first.oid);
      this.#anchor = anchor ?? first;
      return;
    }
    if (sorted.length === 2) {
      this.#anchor = first;
      const comparing = this.compare(path, second.row, second.oid);
      this.#anchor = anchor ?? first;
      await comparing;
      return;
    }
    this.#token += 1;
    this.many = sorted;
    this.pair = null;
    this.against = null;
    this.compared = [];
    this.detail = null;
    this.verification = null;
    this.note = null;
    this.error = null;
    this.loading = false;
    this.row = first.row;
  }

  async select(
    path: string,
    row: number,
    oid: string,
    listing: Listing = 'commit',
  ): Promise<void> {
    this.#anchor = { row, oid };
    this.pair = null;
    this.against = null;
    this.compared = [];
    this.many = [];
    this.row = row;
    this.loading = true;
    this.error = null;

    // Clicking down the graph starts several reads; only the newest may write the result,
    // otherwise a slow earlier one lands last and shows the wrong commit.
    const token = ++this.#token;
    this.verification = null;
    this.note = null;
    // A stash is not signed and has no note, and a failure here costs only the line that
    // would have said so.
    if (listing === 'commit') {
      void commitVerification(path, oid)
        .then((v) => {
          if (token === this.#token) this.verification = v ?? null;
        })
        .catch(() => undefined);
      void this.#readNote(path, oid, token);
    }
    try {
      const detail = await commitDetail(path, oid, listing);
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
    this.many = [];
    this.against = null;
    this.pair = { from, to };
    this.detail = null;
    this.verification = null;
    this.note = null;
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

  /** Compares a commit with the working tree: everything since it, staged or not. */
  async compareWithWorktree(path: string, from: Named): Promise<void> {
    await this.#against(path, { kind: 'worktree', from }, () => changedSince(path, from.oid));
  }

  /**
   * Compares one revision with another, by the names they were asked for by.
   *
   * `sinceTheyParted` measures from where the two parted, which is what "what does `to` add"
   * means: between the tips alone, everything `from` gained since then reads as if `to` had
   * taken it away.
   */
  async compareRefs(path: string, from: Named, to: Named, sinceTheyParted: boolean): Promise<void> {
    await this.#against(path, { kind: 'refs', from, to, base: null }, async () => {
      if (!sinceTheyParted) return compareCommits(path, from.oid, to.oid);
      const base = await mergeBase(path, from.oid, to.oid);
      if (base === null) throw new Error(`${from.name} and ${to.name} share no history.`);
      if (this.against?.kind === 'refs') this.against = { ...this.against, base };
      return compareCommits(path, base, to.oid);
    });
  }

  /** The two revisions the open comparison reads a file's diff between; null for none. */
  get againstRevs(): { from: string; to: string | null } | null {
    const a = this.against;
    if (a === null) return null;
    if (a.kind === 'worktree') return { from: a.from.oid, to: null };
    return { from: a.base ?? a.from.oid, to: a.to.oid };
  }

  async #against(path: string, against: Against, read: () => Promise<ChangedFile[]>): Promise<void> {
    const token = ++this.#token;
    this.#anchor = null;
    this.row = null;
    this.pair = null;
    this.many = [];
    this.detail = null;
    this.verification = null;
    this.note = null;
    this.compared = [];
    this.against = against;
    this.loading = true;
    this.error = null;
    try {
      const files = await read();
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
    // A frame is a window, not the whole graph. One that does not reach where the selection
    // says it is says nothing about it: on a repository worth paging, scrolling away from a
    // selected commit loads a window that does not hold it, and reading that as "the commit
    // is gone" threw the selection away for scrolling.
    if (!this.reaches(frame)) return;
    if (this.many.length > 0) {
      const rows = this.many.map((p) => rowOfOid(frame, p.oid));
      if (rows.some((row) => row === null)) {
        this.clear();
        return;
      }
      // Written only when a row really moved: this runs from an effect that reads `many`, and
      // a new array on every call re-ran it without end and froze the window.
      if (rows.every((row, i) => row === this.many[i]?.row)) return;
      this.many = this.many.map((p, i) => ({ oid: p.oid, row: rows[i] ?? p.row }));
      this.row = this.many[0]?.row ?? null;
      return;
    }
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

  /** Whether `frame` holds the rows the selection claims, and so can speak about them. */
  reaches(frame: Frame): boolean {
    if (this.many.length > 0) return this.many.every((p) => covers(frame, p.row, p.row));
    const pair = this.pair;
    if (pair !== null) {
      return covers(frame, pair.to.row, pair.to.row) && covers(frame, pair.from.row, pair.from.row);
    }
    return this.row !== null && covers(frame, this.row, this.row);
  }

  /** True for a row that is one end of the comparison, so the list can mark both. */
  marks(row: number): boolean {
    if (this.many.length > 0) return this.many.some((p) => p.row === row);
    const pair = this.pair;
    if (pair === null) return this.row === row;
    return pair.from.row === row || pair.to.row === row;
  }
}
