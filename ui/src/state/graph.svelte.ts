import { checkBinaryTransport, graphFrame, rowMetadata } from '../ipc/graph';
import type { CommitMeta } from '../ipc/types';
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

  /** Author and summary for rows that have been on screen, keyed by row number. */
  meta = $state<Map<number, CommitMeta>>(new Map());
  #path = '';
  #inFlight = new Set<number>();

  /**
   * Loads metadata for a window, in blocks, skipping what is already held.
   *
   * The commit-graph carries neither author nor message, so each row here costs an object
   * read. Fetching only what is visible is what keeps scrolling cheap on a large repository.
   */
  async loadMetadata(startRow: number, count: number): Promise<void> {
    if (!this.#path || count <= 0) return;
    const block = 256;
    const first = Math.max(0, Math.floor(startRow / block) * block);
    const last = Math.min(this.totalRows, startRow + count);

    for (let at = first; at < last; at += block) {
      if (this.meta.has(at) || this.#inFlight.has(at)) continue;
      this.#inFlight.add(at);
      try {
        const rows = await rowMetadata(this.#path, at, block);
        const next = new Map(this.meta);
        rows.forEach((m, i) => next.set(at + i, m));
        this.meta = next;
      } catch {
        // A window that fails to load leaves those rows without a summary rather than
        // breaking the graph; scrolling back will retry.
      } finally {
        this.#inFlight.delete(at);
      }
    }
  }

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
    this.meta = new Map();
    this.#path = path;
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
