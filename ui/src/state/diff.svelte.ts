import { fileDiff } from '../ipc/commands';
import type { FileDiff } from '../ipc/types';

export type DiffMode = 'inline' | 'split';

/** The file currently open in the diff viewer. */
export class DiffState {
  file = $state<FileDiff | null>(null);
  /** Path being shown, held separately so the header has something during the load. */
  path = $state<string | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  mode = $state<DiffMode>('inline');

  #token = 0;

  /** Opens one file's diff. A second call supersedes the first. */
  async open(repo: string, rev: string, path: string): Promise<void> {
    const token = ++this.#token;
    this.path = path;
    this.file = null;
    this.error = null;
    this.loading = true;
    try {
      const got = await fileDiff(repo, rev, path);
      // Clicking down a long file list must not let an earlier, slower read win.
      if (token !== this.#token) return;
      this.file = got;
      if (got === null) this.error = 'This commit did not change that file.';
    } catch (e) {
      if (token !== this.#token) return;
      this.error = e instanceof Error ? e.message : String(e);
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
  }
}
