import { repoWorktrees } from '../ipc/commands';
import type { Worktree } from '../ipc/types';
import { messageOf } from '../ipc/error';

/**
 * The repository's working trees.
 *
 * Kept apart from the refs, which is where the other lists in the panel come from: a working
 * tree is not a ref, and a repository can gain or lose one without any ref moving.
 */
export class WorktreesState {
  all = $state<Worktree[]>([]);
  error = $state<string | null>(null);

  /** Which repository is wanted, so an answer for the one being left can be dropped. */
  #path = '';

  /**
   * The trees other than the repository's own.
   *
   * The main tree is always in git's listing and is the only one that cannot be removed, so
   * offering it in a list whose one action is "remove" would be offering nothing. git's own
   * ordering says which it is: comparing paths does not, because inside a submodule git reports
   * the gitdir under `.git/modules/…` rather than the checkout, and the submodule listed itself.
   */
  linked = $derived(this.all.filter((w) => !w.main && !w.bare));

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      const all = await repoWorktrees(path);
      if (this.#path !== path) return;
      this.all = all;
    } catch (e) {
      if (this.#path !== path) return;
      this.error = messageOf(e);
      this.all = [];
    }
  }

  /** Empties the list, for a repository being left. */
  clear(): void {
    this.#path = '';
    this.all = [];
    this.error = null;
  }
}
