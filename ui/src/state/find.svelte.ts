import { searchCommits, stopSearch, type FoundCommit } from '../ipc/graph';
import { codeOf, messageOf } from '../ipc/error';

/** How many matches are worth stepping through. Past this it is a filter, not a search. */
export const FIND_LIMIT = 500;

/**
 * Finding a commit by what is written on it, or by what it changed.
 *
 * The engine matches the message, the author, the paths and the object id, narrowed by any
 * `code:`, `after:` or `before:` in the query, and places each match on the row it sits on;
 * this holds where the reader is among them.
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
  /** Where the search still running went, so closing the box can stop it there. */
  #repo: string | null = null;

  /**
   * Whether the query reads every diff, which on a large history is minutes rather than
   * seconds, so the wait can say why.
   */
  readsChanges = $derived(/(^|\s)code:\S/iu.test(this.query));

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
    this.#stop();
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
      this.#stop();
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
   * Latest wins, and the engine is what makes that cheap. A search of the kernel is a walk of
   * 1.8 million commits, and a `code:` search reads every diff on the way; someone typing a
   * phrase with pauses in it once had thirteen walks in flight for it. Each new search of a
   * repository now stops the one before it there, killing its git, so the stopped one answers
   * `cancelled` and is let go without a word.
   */
  async run(repo: string, query: string): Promise<void> {
    const token = ++this.#token;
    this.#repo = repo;
    this.searching = true;
    this.error = null;
    try {
      const found = await searchCommits(repo, query, FIND_LIMIT);
      if (token !== this.#token) return;
      this.matches = found;
      this.at = 0;
    } catch (e) {
      if (token !== this.#token || codeOf(e) === 'cancelled') return;
      this.error = messageOf(e);
      this.matches = [];
    } finally {
      if (token === this.#token) {
        this.searching = false;
        this.#repo = null;
      }
    }
  }

  /** Stops whatever search is still running, which nobody now wants the answer to. */
  #stop(): void {
    const repo = this.#repo;
    this.#repo = null;
    this.searching = false;
    if (repo !== null) void stopSearch(repo).catch(() => false);
  }

  /** Moves to the next match, or the previous one, wrapping at either end. */
  step(direction: 1 | -1): number | null {
    if (this.matches.length === 0) return null;
    this.at = (this.at + direction + this.matches.length) % this.matches.length;
    return this.current;
  }
}
