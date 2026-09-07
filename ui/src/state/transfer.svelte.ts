/**
 * What the window shows while something is talking to a server.
 *
 * One at a time: actions are serialised per repository already, and a clone happens on the
 * start page where nothing else is running. Holding one makes the bar unambiguous — it is
 * always about the thing that is happening now.
 */

import { cancelTransfer, type TransferReport } from '../ipc/transfer';

export class TransferState {
  /** The transfer in flight, or null when nothing is. */
  current = $state<TransferReport | null>(null);
  /** True between asking to stop and the report that says it stopped. */
  stopping = $state(false);

  /**
   * True once git has said something countable.
   *
   * Until then the bar has nothing to draw and says only that the work has started, which is
   * the honest state while a connection is being made — and the state a host that never
   * answers stays in.
   */
  readonly measured = $derived(this.current !== null && this.current.total > 0);

  /** What to put on the bar. */
  readonly caption = $derived.by(() => {
    const t = this.current;
    if (t === null) return '';
    if (t.phase === '') return `${t.label}…`;
    const where = t.remote ? ' on the server' : '';
    return `${t.label}: ${t.phase}${where}`;
  });

  /** Takes one report. Terminal states clear the bar rather than leaving a finished one up. */
  take(report: TransferReport): void {
    if (report.state === 'running') {
      this.current = report;
      return;
    }
    // Only the transfer we are showing may clear it: a stale report from one that has already
    // been replaced would take the live bar away.
    if (this.current === null || this.current.key === report.key) {
      this.current = null;
      this.stopping = false;
    }
  }

  /** Asks the engine to stop what is running. */
  async cancel(): Promise<void> {
    const t = this.current;
    if (t === null) return;
    this.stopping = true;
    await cancelTransfer(t.key);
  }

  clear(): void {
    this.current = null;
    this.stopping = false;
  }
}
