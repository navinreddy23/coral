import { repoStashes, type PlacedStash } from '../ipc/stash';
import { messageOf } from '../ipc/error';

/** The stash stack, as the sidebar lists it. */
export class StashesState {
  list = $state<PlacedStash[]>([]);
  error = $state<string | null>(null);

  /** Which repository is wanted, so an answer for the one being left can be dropped. */
  #path = '';

  async load(path: string): Promise<void> {
    this.#path = path;
    try {
      const list = await repoStashes(path);
      if (this.#path !== path) return;
      this.list = list;
      this.error = null;
    } catch (e) {
      if (this.#path !== path) return;
      // An empty list rather than a stale one: a stash that has been popped must not stay on
      // screen offering to be popped again.
      this.list = [];
      this.error = messageOf(e);
    }
  }

  clear(): void {
    this.#path = '';
    this.list = [];
    this.error = null;
  }
}
