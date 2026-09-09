import { checkBinaryTransport, graphFrame, graphRewalk, rowMetadata } from '../ipc/graph';
import type { CommitMeta } from '../ipc/types';
import { covers, frameStartFor, FrameFlag, type Frame } from '../graph/frame';
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
  /**
   * What is known about each commit, keyed by its object id rather than by its row.
   *
   * A row is a position in one walk, and the walk is replaced whenever the repository moves.
   * Keyed by row this map had to be emptied every time that happened — which blanked every
   * commit message on screen until the refetch landed, and, if the refetch raced the walk, put
   * one commit's message under another's object id. A commit's own id means the same thing in
   * every walk, so nothing has to be thrown away and nothing can be misread.
   */
  meta = $state<Map<string, CommitMeta>>(new Map());
  /** Blocks already read for the walk on screen, so scrolling does not ask twice. */
  #loaded = new Set<number>();
  #path = '';
  /** Which repository the held frame came from, so a switch can blank it and a reload cannot. */
  #framePath = '';
  /** Set by {@link forget}: the next walk is a different set of commits, not a reload. */
  #shapeChanged = false;
  #inFlight = new Set<number>();
  /** Start row of the frame being fetched, so a scroll does not queue the same one twice. */
  #wantedStart = -1;
  /** The paging fetch in flight, so a second ask for the same window waits for it. */
  #loading: Promise<void> = Promise.resolve();
  /** Which walk the rows on screen came from, so a late read cannot write into the next one. */
  #walk = 0;

  /**
   * Loads metadata for a window, in blocks, skipping what is already held.
   *
   * The commit-graph carries neither author nor message, so each row here costs an object
   * read. Fetching only what is visible is what keeps scrolling cheap on a large repository.
   */
  async loadMetadata(startRow: number, count: number): Promise<void> {
    if (!this.#path || count <= 0) return;
    // Which walk this block belongs to. What it reads is keyed by object id and stays true
    // whatever happens next; it is only the record of having read it that a rewalk invalidates.
    const walk = this.#walk;
    // Which walk the rows on screen came from. The engine holds both for a moment, and a row
    // means a different commit in each.
    const provisional = this.provisional;
    const block = 256;
    const first = Math.max(0, Math.floor(startRow / block) * block);
    const last = Math.min(this.totalRows, startRow + count);

    for (let at = first; at < last; at += block) {
      if (this.#loaded.has(at) || this.#inFlight.has(at)) continue;
      this.#inFlight.add(at);
      try {
        const rows = await rowMetadata(this.#path, at, block, provisional);
        const next = new Map(this.meta);
        for (const m of rows) next.set(m.oid, m);
        this.meta = next;
        if (walk === this.#walk) this.#loaded.add(at);
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
    // More than one pass, because the caller is not the only one asking. Revealing a ref
    // scrolls to its row and then wants the rows before it can select one, and the scroll
    // starts a fetch of its own for a neighbouring window — the compressed scroll range does
    // not convert a row back to exactly itself. Whichever request lands last wins, so a single
    // pass could return holding a frame that does not cover what was asked for, and the caller
    // selected nothing. On the kernel that was a tag in the side panel scrolling into view and
    // staying unselected until it was clicked again.
    for (let attempt = 0; attempt < 3; attempt += 1) {
      if (covers(this.frame, first, last)) return;
      const start = frameStartFor(first, this.totalRows);
      await this.#load(start);
      if (covers(this.frame, first, last)) return;
      // Somebody else's frame won. Forget that this window was asked for, so the next pass
      // asks again rather than waiting on a promise that has already settled.
      if (this.#wantedStart === start) this.#wantedStart = -1;
    }
  }

  /** Fetches one window, sharing the work when the same one is already on its way. */
  async #load(start: number): Promise<void> {
    if (this.#wantedStart === start) {
      await this.#loading;
      return;
    }
    this.#wantedStart = start;
    const path = this.#path;
    const work = (async () => {
      try {
        const next = await graphFrame(path, start, false);
        // A tab switch or a reload may have landed while this was in flight.
        if (this.#wantedStart === start && this.#path === path) this.frame = next;
      } catch (e) {
        if (this.#wantedStart === start) this.#wantedStart = -1;
        this.error = messageOf(e);
      }
    })();
    this.#loading = work;
    await work;
  }

  /**
   * Says the next walk is a different set of commits rather than a reload of this one.
   *
   * So that the next {@link open} paints the fast commit-time screen first instead of waiting
   * for the exact walk. Wanted when the shape is about to change — a branch soloed or hidden,
   * a ref moved, commits pulled in — where the rows on screen are a different question, not a
   * stale answer to this one.
   *
   * It does not blank the frame. Blanking unmounts the scroller, and the row list is
   * positioned against a scroll offset the new element does not have: fast-forwarding a tag
   * while reading row 2,900 of 3,000 drew the rows eighty thousand pixels below the viewport
   * and left an empty pane behind.
   */
  forget(): void {
    this.#shapeChanged = true;
  }

  async open(path: string): Promise<void> {
    this.loading = true;
    this.error = null;
    // A frame belonging to the repository being left has to go, or the window shows one
    // repository's commits under another's name for as long as the walk takes — and the
    // loading screen, which asks whether there is a frame, never appears at all. Staying on
    // the same repository keeps it, however much the walk is about to change: the rows are
    // stale, and stale rows in the place the reader left them beat an empty pane.
    const sameRepository = path === this.#framePath;
    if (!sameRepository) this.frame = null;
    // The fast pass is for rows that are about to mean a different set of commits: arriving at
    // a repository, or after something moved a ref. A plain reload of the same walk skips it,
    // where it would only replace the rows with commit-time order and then replace that again.
    const fast = !sameRepository || this.#shapeChanged;
    this.#shapeChanged = false;
    this.#path = path;
    this.#wantedStart = 0;
    try {
      const check = await checkBinaryTransport();
      this.transportWarning = check.binary ? null : check.detail;

      // Opening means walking. The engine holds one walk per repository and would otherwise
      // hand back the one it already has, which is why a commit made since did not appear.
      await graphRewalk(path);

      // The rows now mean something different, so what has been read for which row does too.
      // What was read is kept: it is keyed by object id, and a commit's message does not
      // change because the walk around it did.
      this.#walk += 1;
      this.#inFlight.clear();
      this.#loaded.clear();

      // The provisional pass is for arriving at a repository, where anything on screen beats a
      // blank one for five seconds. Reopening the one already shown does not need it: the rows
      // are still up, and the pass would only replace them with commit-time order and then
      // replace that again.
      if (fast) {
        const first = await graphFrame(path, 0, true);
        // The repository may have been left while this was being walked. Assigning it anyway
        // is what put one repository's commits under another repository's name, and made the
        // loading screen vanish because there was suddenly a frame to show.
        if (this.#path !== path) return;
        this.frame = first;
        this.#framePath = path;
        // What arrived, not what was asked for. The engine holds the walk between visits, so
        // asking for a quick one can be answered with the topological walk it still has, and
        // saying the rows are out of order when they are not is a badge nobody can dismiss.
        this.provisional = (first.flags & FrameFlag.Provisional) !== 0;
      }

      const full = await graphFrame(path, 0, false);
      if (this.#path !== path) return;
      this.frame = full;
      this.#framePath = path;
      this.provisional = false;
      // The rows have been renumbered, so what was read for which row is no longer true. What
      // was read is kept: it is keyed by object id, so a commit already on screen stays named.
      this.#walk += 1;
      this.#inFlight.clear();
      this.#loaded.clear();
    } catch (e) {
      if (this.#path !== path) return;
      this.error = messageOf(e);
      this.frame = null;
      this.#framePath = '';
    } finally {
      // Only the walk that is still wanted may say the window has stopped loading.
      if (this.#path === path) this.loading = false;
    }
  }
}
