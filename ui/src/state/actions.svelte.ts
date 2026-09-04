import { runAction, type Action, type ActionOutcome } from '../ipc/commands';
import { messageOf } from '../ipc/error';

/** What the status line is showing about the last thing that ran. */
export interface Report {
  text: string;
  tone: 'ok' | 'warn' | 'error';
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
      this.report = outcome.conflicted
        ? { text: `${outcome.what} stopped on conflicts`, tone: 'warn' }
        : { text: outcome.what, tone: 'ok' };
      return outcome;
    } catch (e) {
      this.report = { text: messageOf(e), tone: 'error' };
      return null;
    } finally {
      this.busy = false;
    }
  }

  clear(): void {
    this.report = null;
  }
}
