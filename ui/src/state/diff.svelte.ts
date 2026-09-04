import { fileDiff, worktreeDiff } from '../ipc/commands';
import type { FileDiff } from '../ipc/types';
import type { DiffMode, ViewsState } from './views.svelte';
import { messageOf } from '../ipc/error';

export type { DiffMode };

/** The file currently open in the diff viewer. */
export class DiffState {
  file = $state<FileDiff | null>(null);
  /** Path being shown, held separately so the header has something during the load. */
  path = $state<string | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  #token = 0;
  #views: ViewsState;

  constructor(views: ViewsState) {
    this.#views = views;
  }

  /**
   * Inline or side by side, remembered across launches.
   *
   * Nobody picks side-by-side once and means it for one file. A getter rather than a `$derived`
   * field, because a field initialiser runs before the constructor body.
   */
  get mode(): DiffMode {
    return this.#views.current.diff;
  }

  setMode(mode: DiffMode): void {
    this.#views.set('diff', mode);
  }

  /** What is being shown, so the header can say whether it is a commit or the working tree. */
  source = $state<'commit' | 'unstaged' | 'staged'>('commit');

  /** Opens one file's diff from a commit. A second call supersedes the first. */
  async open(repo: string, rev: string, path: string): Promise<void> {
    await this.#load('commit', path, () => fileDiff(repo, rev, path), 'This commit did not change that file.');
  }

  /**
   * Opens one file's diff in the working tree.
   *
   * A file can be in both lists with different hunks — part of it staged, part not — so which
   * side was clicked decides which diff is shown.
   */
  async openWorking(repo: string, staged: boolean, path: string): Promise<void> {
    await this.#load(
      staged ? 'staged' : 'unstaged',
      path,
      () => worktreeDiff(repo, staged, path),
      absent(staged),
    );
  }

  /**
   * Re-reads the working-tree diff on screen, for a file that changed underneath it.
   *
   * Keeps what is showing until the new answer arrives, unlike opening one: this runs whenever
   * the working tree moves, and blanking the panel first would make an editor's autosave flash
   * it. A commit's diff is not re-read, because a commit does not change.
   */
  async reload(repo: string): Promise<void> {
    const path = this.path;
    if (path === null || this.source === 'commit') return;
    const staged = this.source === 'staged';
    const token = ++this.#token;
    try {
      const got = await worktreeDiff(repo, staged, path);
      if (token !== this.#token) return;
      this.file = got;
      this.error = got === null ? absent(staged) : null;
    } catch (e) {
      if (token !== this.#token) return;
      this.error = messageOf(e);
    }
  }

  async #load(
    source: 'commit' | 'unstaged' | 'staged',
    path: string,
    read: () => Promise<FileDiff | null>,
    absent: string,
  ): Promise<void> {
    const token = ++this.#token;
    this.source = source;
    this.path = path;
    this.file = null;
    this.error = null;
    this.loading = true;
    try {
      const got = await read();
      // Clicking down a long file list must not let an earlier, slower read win.
      if (token !== this.#token) return;
      this.file = got;
      if (got === null) this.error = absent;
    } catch (e) {
      if (token !== this.#token) return;
      this.error = messageOf(e);
    } finally {
      if (token === this.#token) this.loading = false;
    }
  }

  close(): void {
    this.#token++;
    this.file = null;
    this.path = null;
    this.error = null;
    this.loading = false;
    this.source = 'commit';
  }
}

/** What to say when the side being shown has nothing in it for that file. */
function absent(staged: boolean): string {
  return staged ? 'Nothing is staged for that file.' : 'That file has no unstaged changes.';
}
