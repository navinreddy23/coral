import { checkBinaryTransport, graphFrame, rowMetadata } from '../ipc/graph';
import type { CommitMeta } from '../ipc/types';
import { covers, frameStartFor, type Frame } from '../graph/frame';
import { messageOf } from '../ipc/error';

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
  /** Which repository the held frame came from, so a switch can blank it and a reload cannot. */
  #framePath = '';
  #inFlight = new Set<number>();
  /** Start row of the frame being fetched, so a scroll does not queue the same one twice. */
  #wantedStart = -1;

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
  /**
   * Makes sure the loaded frame holds `[first, last]`, fetching another if it does not.
   *
   * A frame is a window, not the whole graph: 1.4M rows of lanes and object ids is far more
   * than the webview should hold, and the engine caps one frame at `ROWS_PER_FRAME`. Rows
   * outside it have no lanes, no object id, and no position, which is what made a ref pointing
   * deep into history scroll to nowhere.
   */
  async ensureRows(first: number, last: number): Promise<void> {
    if (!this.#path || this.totalRows === 0) return;
    if (covers(this.frame, first, last)) return;

    const start = frameStartFor(first, this.totalRows);
    if (this.#wantedStart === start) return;
    this.#wantedStart = start;
    const path = this.#path;
    try {
      const next = await graphFrame(path, start, false);
      // A tab switch or a reload may have landed while this was in flight.
      if (this.#wantedStart === start && this.#path === path) this.frame = next;
    } catch (e) {
      if (this.#wantedStart === start) this.#wantedStart = -1;
      this.error = messageOf(e);
    }
  }

  async open(path: string): Promise<void> {
    this.loading = true;
    this.error = null;
    this.meta = new Map();
    // A frame belonging to the repository being left has to go, or the window shows one
    // repository's commits under another's name for as long as the walk takes — and the
    // loading screen, which asks whether there is a frame, never appears at all. Reopening
    // the same repository keeps it, so a reload after an action does not blank the graph.
    if (path !== this.#framePath) this.frame = null;
    this.#path = path;
    this.#wantedStart = 0;
    try {
      const check = await checkBinaryTransport();
      this.transportWarning = check.binary ? null : check.detail;

      this.frame = await graphFrame(path, 0, true);
      this.#framePath = path;
      this.provisional = true;

      this.frame = await graphFrame(path, 0, false);
      this.provisional = false;
    } catch (e) {
      this.error = messageOf(e);
      this.frame = null;
      this.#framePath = '';
    } finally {
      this.loading = false;
    }
  }
}
