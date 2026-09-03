import { checkBinaryTransport, graphFrame } from '../ipc/graph';
import type { Frame } from '../graph/frame';

/**
 * The graph for one repository.
 *
 * Rows stay inside the decoded frame as typed arrays. Turning them into objects is what makes
 * a large repository unusable in a webview, so nothing here builds a per-row object.
 */
export class GraphState {
  frame = $state<Frame | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  /** Set when the binary IPC path is not working, which the user needs told about. */
  transportWarning = $state<string | null>(null);
  /** True while showing commit-time rows that the topological pass will replace. */
  provisional = $state(false);

  totalRows = $derived(this.frame?.totalRows ?? 0);

  /**
   * Paints quickly, then replaces those rows with topologically correct ones.
   *
   * Two phases is not an optimisation. Topological order has to prepaint the whole graph
   * before it can emit its first row — 1.5 s on the Linux kernel, against a 300 ms budget —
   * while a commit-time walk answers in tens of milliseconds but may place a child before its
   * parent when committer clocks disagree.
   */
  async open(path: string): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      const check = await checkBinaryTransport();
      this.transportWarning = check.binary ? null : check.detail;

      this.frame = await graphFrame(path, 0, true);
      this.provisional = true;

      this.frame = await graphFrame(path, 0, false);
      this.provisional = false;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.frame = null;
    } finally {
      this.loading = false;
    }
  }
}
