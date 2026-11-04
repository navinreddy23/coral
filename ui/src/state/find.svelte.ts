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
  /** Whole seconds the running search has taken, ticking only while one runs. */
  seconds = $state(0);
  /** Set when the reader stopped a search, so the box says why there are no matches. */
  stopped = $state(false);

  /** Which query the held matches answer, so a stale reply cannot replace a newer one. */
  #token = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  /** Where the search still running went, so closing the box can stop it there. */
  #repo: string | null = null;
  #clock: ReturnType<typeof setInterval> | null = null;

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
    this.stopped = false;
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
    this.stopped = false;
    this.error = null;
    this.#startClock();
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
        this.#stopClock();
      }
    }
  }

  /**
   * Stops the running search and keeps the query, for a `code:` search that is taking longer
   * than the answer is worth.
   */
  stop(): void {
    if (!this.searching) return;
    this.#token += 1;
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#stop();
    this.matches = [];
    this.at = 0;
    this.stopped = true;
  }

  #startClock(): void {
    this.#stopClock();
    const started = Date.now();
    this.#clock = setInterval(() => {
      this.seconds = Math.floor((Date.now() - started) / 1000);
    }, 1000);
  }

  #stopClock(): void {
    if (this.#clock !== null) clearInterval(this.#clock);
    this.#clock = null;
    this.seconds = 0;
  }

  /** Stops whatever search is still running, which nobody now wants the answer to. */
  #stop(): void {
    const repo = this.#repo;
    this.#repo = null;
    this.searching = false;
    this.#stopClock();
    if (repo !== null) void stopSearch(repo).catch(() => false);
  }

  /** Moves to the next match, or the previous one, wrapping at either end. */
  step(direction: 1 | -1): number | null {
    if (this.matches.length === 0) return null;
    this.at = (this.at + direction + this.matches.length) % this.matches.length;
    return this.current;
  }
}

/** A `code:` qualifier, with its value quoted or bare. */
const CODE = /(^|\s)code:("[^"]*"|\S+)/iu;
/** The qualifiers that stay where they are when the text moves in or out of `code:`. */
const KEPT = /^(?:after|before):\S/iu;

/**
 * The query with its words searched for in what each commit changed, or taken back out.
 *
 * On, the free words become one `code:"…"` and `after:`/`before:` stay; off, the `code:`
 * value goes back to being free text, so toggling twice gives back what was typed.
 */
export function inChanges(query: string, on: boolean): string {
  const found = CODE.exec(query);
  if (!on) {
    if (found === null) return query;
    const value = (found[2] ?? '').replace(/^"(.*)"$/u, '$1').replace(/^\/(.*)\/$/u, '$1');
    return `${query.slice(0, found.index)}${found[1] ?? ''}${value}${query.slice(found.index + found[0].length)}`
      .replace(/\s+/gu, ' ')
      .trim();
  }
  if (found !== null) return query;
  const words = query.trim().split(/\s+/u).filter((w) => w !== '');
  const kept = words.filter((w) => KEPT.test(w));
  const text = words.filter((w) => !KEPT.test(w)).join(' ');
  if (text === '') return [...kept, 'code:'].join(' ');
  const value = /\s|"/u.test(text) ? `"${text.replace(/"/gu, '')}"` : text;
  return [`code:${value}`, ...kept].join(' ');
}
