import {
  applyPart,
  commitStaged,
  deletePaths,
  discardPaths,
  repoStatus,
  stagePaths,
  type Part,
} from '../ipc/commands';

import type { Status, StatusEntry } from '../ipc/types';
import { messageOf } from '../ipc/error';

/**
 * The working tree: what the WIP row summarises and the staging panel acts on.
 *
 * Every mutation returns the new status rather than nothing, so the panel cannot drift from
 * the repository — there is no separate refresh to forget.
 */
export class WorktreeState {
  status = $state<Status | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  #path = '';

  /** Changes that are in the index and would go into a commit. */
  staged = $derived<StatusEntry[]>(
    (this.status?.entries ?? []).filter(
      (e) => e.index !== 'unmodified' && e.index !== 'untracked' && e.index !== 'ignored',
    ),
  );

  /** Changes in the worktree that are not staged, including untracked files. */
  unstaged = $derived<StatusEntry[]>(
    (this.status?.entries ?? []).filter((e) => e.worktree !== 'unmodified' && !e.conflict),
  );

  conflicted = $derived<StatusEntry[]>(
    (this.status?.entries ?? []).filter((e) => e.conflict !== null),
  );

  dirty = $derived((this.status?.entries.length ?? 0) > 0);

  async load(path: string): Promise<void> {
    this.#path = path;
    await this.#run(() => repoStatus(path));
  }

  /**
   * Throws away changes: `restore` back to HEAD, `remove` deleted from disk.
   *
   * Nothing is worked out here. The dialog that asked the user is the only thing that knows
   * what they agreed to, so it hands both lists over exactly as it described them.
   */
  async discard(restore: string[], remove: string[]): Promise<void> {
    if (restore.length === 0 && remove.length === 0) return;
    const path = this.#path;
    await this.#run(() => discardPaths(path, restore, remove));
  }

  async stage(paths: string[], stage: boolean): Promise<void> {
    if (paths.length === 0) return;
    await this.#run(() => stagePaths(this.#path, paths, stage));
  }

  /** Stages, unstages or discards one hunk of a file, or only some of its lines. */
  async applyPart(file: string, part: Part, hunk: number, lines: number[]): Promise<void> {
    const path = this.#path;
    await this.#run(() => applyPart(path, file, part, hunk, lines));
  }

  /** Deletes files outright. The caller asks first; this does not. */
  async delete(tracked: string[], untracked: string[]): Promise<void> {
    if (tracked.length === 0 && untracked.length === 0) return;
    const path = this.#path;
    await this.#run(() => deletePaths(path, tracked, untracked));
  }

  async commit(message: string, amend = false): Promise<void> {
    if (message.trim().length === 0) return;
    await this.#run(() =>
      commitStaged(this.#path, message, amend),
    );
  }

  async #run(action: () => Promise<Status>): Promise<void> {
    if (!this.#path) return;
    // Which repository this belongs to. A status for the one being left answers after the
    // switch and would otherwise be shown under the new one's name.
    const path = this.#path;
    this.busy = true;
    this.error = null;
    try {
      const status = await action();
      if (this.#path !== path) return;
      this.status = status;
    } catch (e) {
      if (this.#path !== path) return;
      this.error = messageOf(e);
    } finally {
      if (this.#path === path) this.busy = false;
    }
  }

  /** Empties the panel, for a repository being left. */
  clear(): void {
    this.#path = '';
    this.status = null;
    this.error = null;
  }
}
