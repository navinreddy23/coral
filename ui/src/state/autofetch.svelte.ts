import type { AutoFetch } from '../ipc/types';

const STORAGE_KEY = 'coral.autoFetch';

/** What Preferences offers, in minutes. Nought is never. */
export const INTERVALS = [0, 5, 10, 30] as const;

const DEFAULT_MINUTES = 10;

/** An interval as a choice reads: "Never", "Every minute", "Every 10 minutes". */
export function intervalWords(minutes: number): string {
  if (minutes === 0) return 'Never';
  return minutes === 1 ? 'Every minute' : `Every ${minutes} minutes`;
}

/**
 * How often the timer looks, not how often it fetches.
 *
 * Coarse because nothing is lost by a fetch half a minute late, and a finer clock is a finer
 * clock running in every window all day.
 */
const TICK_MS = 30_000;

function stored(): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const minutes = raw === null ? DEFAULT_MINUTES : Number(raw);
    return (INTERVALS as readonly number[]).includes(minutes) ? minutes : DEFAULT_MINUTES;
  } catch {
    return DEFAULT_MINUTES;
  }
}

/**
 * Fetches the repository in front of you every few minutes, so what the graph says about the
 * remote is never more than that old.
 *
 * Only the active tab, and only while the window has focus: a fetch is a graph rewalk when it
 * brings anything, and rewalking repositories nobody is looking at is a cost with nobody to
 * see what it bought. Coming back to the window after the interval has passed fetches then,
 * which is the moment the answer is wanted.
 *
 * Counted from the last fetch of any kind, the user's own included, and from opening the
 * repository rather than at once: opening already walks the graph, and fetching on top of
 * that would walk it twice before anyone had read a row.
 */
export class AutoFetchState {
  /** The window's own interval in minutes, for every repository that does not set one. */
  minutes = $state(stored());
  /** What the repository in front of you asks for instead, from its config. */
  override = $state<AutoFetch | null>(null);
  /** Why the last background fetch failed, until one gets through. */
  failure = $state<string | null>(null);

  /** The interval that applies, in minutes. Nought is never. */
  readonly every = $derived.by(() => {
    const o = this.override;
    if (o === null) return this.minutes;
    return o.kind === 'off' ? 0 : o.minutes;
  });

  #path: string | null = null;
  #run: ((path: string) => Promise<void>) | null = null;
  #last = new Map<string, number>();
  #running = false;
  #timer: ReturnType<typeof setInterval> | null = null;
  #focused = (): void => void this.tick();

  setMinutes(minutes: number): void {
    this.minutes = minutes;
    try {
      localStorage.setItem(STORAGE_KEY, String(minutes));
    } catch {
      // Storage refused: the choice holds for this session and the default comes back next.
    }
  }

  /**
   * Follows the repository in front of you, or nothing when `path` is null.
   *
   * `run` does the fetch. It is the window's to provide because only the window knows how
   * to run an action without getting in the way of the one the user is about to start.
   */
  follow(path: string | null, run: (path: string) => Promise<void>): void {
    this.#path = path;
    this.#run = run;
    if (path !== null && !this.#last.has(path)) this.#last.set(path, Date.now());
    if (this.#timer === null) {
      this.#timer = setInterval(this.#focused, TICK_MS);
      window.addEventListener('focus', this.#focused);
    }
  }

  /** Restarts the count for `path`: it has just been fetched, by whoever. */
  fetched(path: string): void {
    this.#last.set(path, Date.now());
  }

  /** Fetches now if the interval has passed and nothing is in the way. */
  async tick(): Promise<void> {
    const path = this.#path;
    const run = this.#run;
    if (path === null || run === null || this.#running || this.every === 0) return;
    if (!document.hasFocus()) return;
    const last = this.#last.get(path) ?? 0;
    if (Date.now() - last < this.every * 60_000) return;
    this.#running = true;
    // Before the fetch rather than after, so a fetch that hangs until the user stops it is
    // not tried again on the next tick and the one after.
    this.fetched(path);
    try {
      await run(path);
    } finally {
      this.#running = false;
    }
  }

  /**
   * Records how a background fetch went, answering whether to say so.
   *
   * Only the first failure in a row is worth a word. A remote that is down stays down for a
   * while, and saying so every ten minutes is a status line nobody can use for anything else.
   */
  settled(failure: string | null): boolean {
    const fresh = failure !== null && this.failure === null;
    this.failure = failure;
    return fresh;
  }

  stop(): void {
    if (this.#timer !== null) clearInterval(this.#timer);
    this.#timer = null;
    window.removeEventListener('focus', this.#focused);
    this.#path = null;
    this.#run = null;
  }
}
