import { repoReflog } from '../ipc/reflog';
import type { ReflogEntry } from '../ipc/types';
import { messageOf } from '../ipc/error';

/**
 * Where HEAD has been, for finding a commit that a reset, a rebase or a deleted branch left
 * behind.
 *
 * Read when the sheet opens and not kept up to date behind it: it is opened to look for one
 * thing, and refreshing it while it is read would move the row being looked at.
 */
export class ReflogState {
  entries = $state<ReflogEntry[]>([]);
  /** Shows only the moves to commits nothing else reaches, which is what is worth recovering. */
  onlyLost = $state(false);
  loading = $state(false);
  error = $state<string | null>(null);

  shown = $derived(this.onlyLost ? this.entries.filter((e) => e.lost) : this.entries);
  /** Distinct lost commits: HEAD is often moved to the same one more than once. */
  lostCount = $derived(new Set(this.entries.filter((e) => e.lost).map((e) => e.oid)).size);

  #path = '';

  async load(path: string): Promise<void> {
    this.#path = path;
    this.loading = true;
    this.error = null;
    try {
      const entries = await repoReflog(path);
      if (this.#path !== path) return;
      this.entries = entries;
    } catch (e) {
      if (this.#path !== path) return;
      this.entries = [];
      this.error = messageOf(e);
    } finally {
      if (this.#path === path) this.loading = false;
    }
  }

  async reload(): Promise<void> {
    if (this.#path) await this.load(this.#path);
  }
}
