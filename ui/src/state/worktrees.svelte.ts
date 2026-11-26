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
  /** The path `all` was listed for. */
  #listed = $state('');

  /**
   * The trees other than the one open here.
   *
   * In the main tree that is every linked one. git's ordering says which tree is the main one:
   * comparing paths does not, because inside a submodule git reports the gitdir under
   * `.git/modules/…` rather than the checkout, and the submodule listed itself. A linked tree is
   * reported at its own path, so a tab open on one is found by it and lists the main tree
   * instead: listing itself offered to remove the folder it showed, which git does.
   */
  others = $derived.by(() => {
    const here = this.all.find((w) => !w.main && w.path === this.#listed);
    return this.all.filter((w) => !w.bare && (here === undefined ? !w.main : w !== here));
  });

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      const all = await repoWorktrees(path);
      if (this.#path !== path) return;
      this.all = all;
      this.#listed = path;
    } catch (e) {
      if (this.#path !== path) return;
      this.error = messageOf(e);
      this.all = [];
    }
  }

  /** Empties the list, for a repository being left. */
  clear(): void {
    this.#path = '';
    this.#listed = '';
    this.all = [];
    this.error = null;
  }
}
