import { commitStaged, repoStatus, stagePaths } from '../ipc/commands';

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

  async stage(paths: string[], stage: boolean): Promise<void> {
    if (paths.length === 0) return;
    await this.#run(() => stagePaths(this.#path, paths, stage));
  }

  async commit(message: string, amend = false): Promise<void> {
    if (message.trim().length === 0) return;
    await this.#run(() =>
      commitStaged(this.#path, message, amend),
    );
  }

  async #run(action: () => Promise<Status>): Promise<void> {
    if (!this.#path) return;
    this.busy = true;
    this.error = null;
    try {
      this.status = await action();
    } catch (e) {
      this.error = messageOf(e);
    } finally {
      this.busy = false;
    }
  }
}
