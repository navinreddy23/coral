import { lfsLock, lfsLocks, lfsUnlock, lfsTrack } from '../ipc/commands';
import { messageOf } from '../ipc/error';
import type { LfsLock } from '../ipc/types';

/**
 * Files locked on the repository's LFS server.
 *
 * Read when a repository opens and after every lock or unlock, not on every refresh: the list
 * lives on a server, and the working copy changing says nothing about it. A repository that
 * keeps nothing in LFS is never asked about at all.
 */
export class LfsState {
  /** The locks, or null for a repository with nothing in LFS or not read yet. */
  locks = $state<LfsLock[] | null>(null);
  /** Why the locks could not be read, kept apart from the list so a stale one is not shown. */
  error = $state<string | null>(null);
  busy = $state(false);

  #path = '';
  #token = 0;

  /** The lock on each locked path. */
  byPath = $derived(new Map((this.locks ?? []).map((lock) => [lock.path, lock])));

  async load(path: string): Promise<void> {
    this.#path = path;
    const token = ++this.#token;
    try {
      const locks = await lfsLocks(path);
      if (token !== this.#token) return;
      this.locks = locks ?? null;
      this.error = null;
    } catch (e) {
      if (token !== this.#token) return;
      this.locks = null;
      this.error = messageOf(e);
    }
  }

  /** Locks, unlocks or tracks, then reads the locks again; the server's refusal, or null. */
  async lock(file: string): Promise<string | null> {
    return this.#run(() => lfsLock(this.#path, file));
  }

  async unlock(file: string, force: boolean): Promise<string | null> {
    return this.#run(() => lfsUnlock(this.#path, file, force));
  }

  async track(pattern: string): Promise<string | null> {
    return this.#run(() => lfsTrack(this.#path, pattern));
  }

  async #run(step: () => Promise<void>): Promise<string | null> {
    this.busy = true;
    try {
      await step();
      return null;
    } catch (e) {
      return messageOf(e);
    } finally {
      this.busy = false;
      await this.load(this.#path);
    }
  }
}
