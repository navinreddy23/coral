import { searchCommits, type FoundCommit } from '../ipc/graph';
import { messageOf } from '../ipc/error';

/** How many matches are worth stepping through. Past this it is a filter, not a search. */
export const FIND_LIMIT = 500;

/**
 * Finding a commit by what is written on it.
 *
 * The engine matches the message, the author and the object id, and places each match on the
 * row it sits on; this holds where the reader is among them.
 */
export class FindState {
  open = $state(false);
  query = $state('');
  matches = $state<FoundCommit[]>([]);
  /** Which match is current, as an index into `matches`. */
  at = $state(0);
  searching = $state(false);
  error = $state<string | null>(null);

  /** Which query the held matches answer, so a stale reply cannot replace a newer one. */
  #token = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  /** Whether a search is out with the engine, and the newest query waiting behind it. */
  #running = false;
  #queued: { repo: string; query: string } | null = null;

  /** The rows that matched, for the list to mark. */
  rows = $derived(new Set(this.matches.map((m) => m.row)));

  /** The row the reader is on, or null when nothing matched. */
  get current(): number | null {
    return this.matches[this.at]?.row ?? null;
  }

  show(): void {
    this.open = true;
  }

  close(): void {
    this.open = false;
    this.clear();
  }

  clear(): void {
    this.#token += 1;
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#queued = null;
    this.query = '';
    this.matches = [];
    this.at = 0;
    this.searching = false;
    this.error = null;
  }

  /**
   * Runs the search a moment after the typing stops.
   *
   * A search of a repository this size is a walk of its whole history, and one per keystroke
   * would queue a dozen of them to answer the one that matters.
   */
  type(repo: string, query: string): void {
    this.query = query;
    if (this.#timer !== null) clearTimeout(this.#timer);
    if (query.trim() === '') {
      this.#token += 1;
      this.#queued = null;
      this.matches = [];
      this.at = 0;
      this.searching = false;
      return;
    }
    this.searching = true;
    this.#timer = setTimeout(() => void this.run(repo, query), 250);
  }

  /**
   * Searches now, for Enter and for the tests.
   *
   * One at a time, latest wins. The debounce only collapses keystrokes closer together than a
   * quarter of a second; someone typing a phrase with a pause in the middle sent one search per
   * pause, and a search of the kernel is a walk of 1.8 million commits. Thirteen of them were
   * once in flight at once for one typed phrase, and the answer to the last had to wait behind
   * the twelve nobody wanted. The engine has no way to call one off, so the queue is held here
   * and everything but the newest is dropped before it is ever asked for.
   */
  async run(repo: string, query: string): Promise<void> {
    if (this.#running) {
      this.#queued = { repo, query };
      this.#token += 1;
      this.searching = true;
      return;
    }
    const token = ++this.#token;
    this.#running = true;
    this.searching = true;
    this.error = null;
    try {
      const found = await searchCommits(repo, query, FIND_LIMIT);
      if (token !== this.#token) return;
      this.matches = found;
      this.at = 0;
    } catch (e) {
      if (token !== this.#token) return;
      this.error = messageOf(e);
      this.matches = [];
    } finally {
      this.#running = false;
      const next = this.#queued;
      this.#queued = null;
      if (next !== null) {
        void this.run(next.repo, next.query);
      } else if (token === this.#token) {
        this.searching = false;
      }
    }
  }

  /** Moves to the next match, or the previous one, wrapping at either end. */
  step(direction: 1 | -1): number | null {
    if (this.matches.length === 0) return null;
    this.at = (this.at + direction + this.matches.length) % this.matches.length;
    return this.current;
  }
}
