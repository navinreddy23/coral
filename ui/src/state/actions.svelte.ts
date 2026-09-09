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

  clear(): void {
    this.report = null;
  }
}
