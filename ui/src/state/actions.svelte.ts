import { runAction, type Action, type ActionOutcome } from '../ipc/commands';
import { messageOf, whatFailed } from '../ipc/error';
import { outcomeToast, type ToastKind } from './toasts.svelte';

/** What the status line is showing about the last thing that ran. */
export interface Report {
  text: string;
  tone: 'ok' | 'warn' | 'error';
  /** The name the engine gave the operation, for a failure that belongs to one. */
  what: string | null;
}

/** The status line has three tones where a toast has four; nothing happening reads as fine. */
function toneOf(kind: ToastKind): Report['tone'] {
  if (kind === 'error') return 'error';
  return kind === 'warn' ? 'warn' : 'ok';
}

/**
 * Runs repository actions one at a time.
 *
 * Serialised deliberately: two mutations at once contend for `index.lock` and the second fails
 * with a message about a lock file, which tells the user nothing about what they did.
 */
export class ActionsState {
  busy = $state(false);
  report = $state<Report | null>(null);
  /** True while something the window started on its own is running. */
  quiet = $state(false);
  #quiet: Promise<unknown> | null = null;

  /**
   * Runs `action`, then hands back whether anything changed so the caller can reload.
   *
   * Reloading is the caller's job rather than this module's: what needs refreshing after a
   * push is not what needs refreshing after a checkout, and only the shell knows what it holds.
   */
  async run(path: string, action: Action): Promise<ActionOutcome | null> {
    if (this.busy) return null;
    this.busy = true;
    this.report = null;
    try {
      // A background fetch finishes first, rather than the user's action racing it for the
      // same refs. It is on the transfer bar meanwhile, with the button that stops it. Only
      // awaited when there is one: an await of nothing still defers the call a turn.
      if (this.#quiet !== null) await this.#quiet;
      const outcome = await runAction(path, action);
      // The same wording as the toast, from the module that owns it. Phrased here as well,
      // the line said "push v1.0 stopped on conflicts" for a rejected push while the toast
      // beside it said "was rejected", which is what actually happened.
      const said = outcomeToast(action.kind, outcome.what, outcome.message, outcome.conflicted);
      this.report = { text: said.title, tone: toneOf(said.kind), what: null };
      return outcome;
    } catch (e) {
      this.report = { text: messageOf(e), tone: 'error', what: whatFailed(e) };
      return null;
    } finally {
      this.busy = false;
    }
  }

  /**
   * Runs something the window decided to do, not the user: a fetch on a timer.
   *
   * Holds nothing the user is waiting on — no `busy`, no status line, no toast — so a click
   * during it is never refused. Answers null when it did not run because something else was;
   * a failure is thrown for the caller to decide whether it is worth a word.
   */
  async quietly(path: string, action: Action): Promise<ActionOutcome | null> {
    if (this.busy || this.#quiet !== null) return null;
    const running = runAction(path, action);
    this.#quiet = running.catch(() => undefined);
    this.quiet = true;
    try {
      return await running;
    } finally {
      this.#quiet = null;
      this.quiet = false;
    }
  }

  clear(): void {
    this.report = null;
  }
}
