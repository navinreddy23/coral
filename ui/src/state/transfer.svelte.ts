/**
 * What the window shows while something is talking to a server.
 *
 * One at a time: actions are serialised per repository already, and a clone happens on the
 * start page where nothing else is running. Holding one makes the bar unambiguous — it is
 * always about the thing that is happening now.
 */

import { cancelTransfer, type TransferReport } from '../ipc/transfer';

export class TransferState {
  /**
   * Every transfer in flight, newest last.
   *
   * A list rather than one, because a clone is started from the start page and a fetch from
   * the toolbar, and those two can overlap. Holding only the latest left the other running
   * with no bar and therefore no way to stop it, which is the one thing this exists to give.
   */
  running = $state<TransferReport[]>([]);
  /** The keys asked to stop, until the report that says they did. */
  stopping = $state<string[]>([]);

  /** The one on the bar: the most recent, since that is what the user just set going. */
  readonly current = $derived(this.running.at(-1) ?? null);

  readonly waiting = $derived(this.running.length - 1);

  /**
   * True once git has said something countable.
   *
   * Until then the bar has nothing to draw and says only that the work has started, which is
   * the honest state while a connection is being made — and the state a host that never
   * answers stays in.
   */
  readonly measured = $derived(this.current !== null && this.current.total > 0);

  /** True between asking this one to stop and the report that says it stopped. */
  readonly asked = $derived(
    this.current !== null && this.stopping.includes(this.current.key),
  );

  /** What to put on the bar. */
  readonly caption = $derived.by(() => {
    const t = this.current;
    if (t === null) return '';
    if (t.phase === '') return `${t.label}…`;
    const where = t.remote ? ' on the server' : '';
    return `${t.label}: ${t.phase}${where}`;
  });

  /** Takes one report. A terminal state removes that transfer, whichever one it was. */
  take(report: TransferReport): void {
    const at = this.running.findIndex((t) => t.key === report.key);
    if (report.state === 'running') {
      if (at < 0) this.running = [...this.running, report];
      else this.running = this.running.map((t, i) => (i === at ? report : t));
      return;
    }
    this.running = this.running.filter((t) => t.key !== report.key);
    this.stopping = this.stopping.filter((k) => k !== report.key);
  }

  /** Asks the engine to stop the one on the bar. */
  async cancel(): Promise<void> {
    const t = this.current;
    if (t === null || this.stopping.includes(t.key)) return;
    this.stopping = [...this.stopping, t.key];
    try {
      await cancelTransfer(t.key);
    } catch {
      // The button must not be left disabled over a failed ask; the transfer is still there
      // and still stoppable, so the honest thing is to offer it again.
      this.stopping = this.stopping.filter((k) => k !== t.key);
    }
  }

  clear(): void {
    this.running = [];
    this.stopping = [];
  }
}
