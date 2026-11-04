import { hostingEverywhere, type RepoRequests } from '../ipc/commands';
import { messageOf } from '../ipc/error';

/**
 * The requests still open across every repository in a tab, for the Start page: what is
 * waiting on somebody, wherever it is, without opening each repository to find out.
 */
export class RequestsState {
  rows = $state<RepoRequests[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  /** When the list was last read, so the page can say how fresh it is. */
  readAt = $state<number | null>(null);

  #token = 0;

  /** How many are open in all, across the repositories that could be asked. */
  total = $derived(this.rows.reduce((n, r) => n + r.requests.length, 0));

  async load(): Promise<void> {
    const token = ++this.#token;
    this.loading = true;
    this.error = null;
    try {
      const rows = await hostingEverywhere();
      if (token !== this.#token) return;
      this.rows = rows;
      this.readAt = Date.now();
    } catch (e) {
      if (token === this.#token) this.error = messageOf(e);
    } finally {
      if (token === this.#token) this.loading = false;
    }
  }
}
