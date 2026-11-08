import {
  applyPart,
  stageEdited,
  commitStaged,
  deletePaths,
  discardPaths,
  repoStatus,
  takePaths,
  type TakeFrom,
  stagePaths,
  type Part,
  type ReadOptions,
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

  /**
   * Changes that are in the index and would go into a commit.
   *
   * A conflicted path is not one of them, however its index column reads: it has stages rather
   * than a resolution, nothing will commit until it is settled, and it is already listed under
   * its own heading. Counting it here listed the same file twice and put it in the tally on
   * the commit button.
   */
  staged = $derived<StatusEntry[]>(
    (this.status?.entries ?? []).filter(
      (e) =>
        !e.conflict &&
        e.index !== 'unmodified' &&
        e.index !== 'untracked' &&
        e.index !== 'ignored',
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

  /** Writes files into the working tree as a commit or a stash has them, staging nothing. */
  async take(from: TakeFrom, files: string[]): Promise<void> {
    if (files.length === 0) return;
    const path = this.#path;
    await this.#run(() => takePaths(path, from, files));
  }

  async stage(paths: string[], stage: boolean): Promise<void> {
    if (paths.length === 0) return;
    await this.#run(() => stagePaths(this.#path, paths, stage));
  }

  /** Stages, unstages or discards one hunk of a file, or only some of its lines. */
  /** `options` are the ones the hunks were read with, so both sides count them alike. */
  async applyPart(
    file: string,
    part: Part,
    hunk: number,
    lines: number[],
    options: ReadOptions,
  ): Promise<void> {
    const path = this.#path;
    await this.#run(() => applyPart(path, file, part, hunk, lines, options));
  }

  /** Stages one hunk as rewritten in `text`, its new side, leaving the working tree alone. */
  async stageEdited(file: string, hunk: number, text: string, options: ReadOptions): Promise<void> {
    const path = this.#path;
    await this.#run(() => stageEdited(path, file, hunk, text, options));
  }

  /** Deletes files outright. The caller asks first; this does not. */
  async delete(tracked: string[], untracked: string[]): Promise<void> {
    if (tracked.length === 0 && untracked.length === 0) return;
    const path = this.#path;
    await this.#run(() => deletePaths(path, tracked, untracked));
  }

  async commit(message: string, amend = false, signoff = false, coAuthors: string[] = []): Promise<void> {
    if (message.trim().length === 0) return;
    await this.#run(() => commitStaged(this.#path, message, amend, null, signoff, coAuthors));
  }

  /**
   * Records what is staged as a fixup of an earlier commit, for the next interactive rebase
   * to fold in. No message: git writes `fixup! <its subject>`, and a fixup's own message is
   * thrown away when it is folded.
   */
  async fixup(target: string): Promise<void> {
    await this.#run(() => commitStaged(this.#path, '', false, { how: 'fixup', target }));
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
