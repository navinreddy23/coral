import { commitDetail } from '../ipc/commands';

import type { CommitDetail } from '../ipc/types';

/**
 * The selected commit and its details.
 *
 * Details are fetched per selection rather than held for every row: the file list of a large
 * merge is thousands of entries, and the graph has a million rows.
 */
export class SelectionState {
  row = $state<number | null>(null);
  detail = $state<CommitDetail | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  #token = 0;

  clear(): void {
    this.row = null;
    this.detail = null;
    this.error = null;
  }

  async select(path: string, row: number, oid: string): Promise<void> {
    this.row = row;
    this.loading = true;
    this.error = null;

    // Clicking down the graph starts several reads; only the newest may write the result,
    // otherwise a slow earlier one lands last and shows the wrong commit.
    const token = ++this.#token;
    try {
      const detail = await commitDetail(path, oid);
      if (token === this.#token) this.detail = detail;
    } catch (e) {
      if (token === this.#token) {
        this.error = e instanceof Error ? e.message : String(e);
        this.detail = null;
      }
    } finally {
      if (token === this.#token) this.loading = false;
    }
  }
}
