/** How a toast reads: what happened, and whether it needs attention. */
export type ToastKind = 'ok' | 'info' | 'warn' | 'error';

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  /** git's own words, when there are any. Shown under the title. */
  detail: string;
}

/** How long each kind stays before dismissing itself, in milliseconds. */
const LIFETIME: Record<ToastKind, number> = {
  ok: 4000,
  info: 5000,
  // A failure has to survive looking away from the window, so it waits to be dismissed.
  warn: 0,
  error: 0,
};

/**
 * The notifications stacked in the corner.
 *
 * They stack rather than replace: a fetch of three remotes reports three times, and the second
 * answer overwriting the first would lose the one that failed.
 */
export class ToastsState {
  items = $state<Toast[]>([]);
  #next = 1;
  #timers = new Map<number, ReturnType<typeof setTimeout>>();

  /** How many are shown at once. Older ones fall off the top rather than filling the window. */
  static readonly MAX = 5;

  push(kind: ToastKind, title: string, detail = ''): number {
    const id = this.#next++;
    this.items = [...this.items, { id, kind, title, detail }].slice(-ToastsState.MAX);

    const life = LIFETIME[kind];
    if (life > 0) {
      this.#timers.set(
        id,
        setTimeout(() => this.dismiss(id), life),
      );
    }
    return id;
  }

  dismiss(id: number): void {
    const timer = this.#timers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      this.#timers.delete(id);
    }
    this.items = this.items.filter((t) => t.id !== id);
  }

  clear(): void {
    for (const timer of this.#timers.values()) clearTimeout(timer);
    this.#timers.clear();
    this.items = [];
  }
}

/**
 * Reads an outcome git reported and decides what to say about it.
 *
 * "Already up to date" is the case worth naming: it is a success, but reporting it as one
 * indistinguishable from a pull that brought commits leaves the user unsure whether anything
 * happened.
 */
export function describe(
  what: string,
  message: string,
  conflicted: boolean,
): { kind: ToastKind; title: string; detail: string } {
  const text = message.trim();
  if (conflicted) {
    // A push that comes back rejected has not conflicted with anything: the branch moved on
    // the remote while this one was being written, and the answer is to fetch, not to resolve.
    const rejected = /\[rejected]|\[remote rejected]|non-fast-forward/i.test(text);
    if (rejected) return { kind: 'warn', title: `${what} was rejected`, detail: text };
    return { kind: 'warn', title: `${what} stopped on conflicts`, detail: whatIsLeftToSay(text) };
  }
  // git has two wordings for "nothing happened": merge and pull say "Already up to date.",
  // push says "Everything up-to-date". Both mean the same thing to the person reading it.
  if (/(?:already|everything) up[- ]to[- ]date/i.test(text)) {
    return { kind: 'info', title: `${what}: already up to date`, detail: '' };
  }
  return { kind: 'ok', title: `${what} complete`, detail: text };
}

/**
 * What to say about an action that has finished.
 *
 * Undo and redo answer with a whole sentence — "undid commit" — where every other action
 * answers with a noun this completes. Put through the same wording they read "undid commit
 * complete".
 */
export function outcomeToast(
  action: string,
  what: string,
  message: string,
  conflicted: boolean,
): { kind: ToastKind; title: string; detail: string } {
  if (action === 'undo' || action === 'redo') {
    return { kind: 'ok', title: what, detail: '' };
  }
  return describe(what, message, conflicted);
}

/**
 * What is worth keeping from git's account of a stop on conflicts.
 *
 * The window answers a stop by opening the merge tool on the files that caused it, so the
 * toast beside it does not need `error: could not revert 02c9e47… Add a page about coffee` —
 * a failure reported where there was none, in git's words, restating the title above it. What
 * survives is what the title does not carry, which in practice is git's CONFLICT lines naming
 * the files.
 */
function whatIsLeftToSay(text: string): string {
  return text
    .split('\n')
    .filter((line) => !/^\s*(?:error|hint|fatal):/iu.test(line))
    .join('\n')
    .trim();
}
