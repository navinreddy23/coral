import { repoStashes, type PlacedStash } from '../ipc/stash';
import { messageOf } from '../ipc/error';

/** The stash stack, as the sidebar lists it. */
export class StashesState {
  list = $state<PlacedStash[]>([]);
  error = $state<string | null>(null);

  async load(path: string): Promise<void> {
    try {
      this.list = await repoStashes(path);
      this.error = null;
    } catch (e) {
      // An empty list rather than a stale one: a stash that has been popped must not stay on
      // screen offering to be popped again.
      this.list = [];
      this.error = messageOf(e);
    }
  }

  clear(): void {
    this.list = [];
    this.error = null;
  }
}
