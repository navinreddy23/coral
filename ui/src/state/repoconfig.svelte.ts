import {
  repoConfig,
  repoDisableSparse,
  repoSetFsmonitor,
  repoSetSparse,
  repoSetTracker,
  repoSetAutoFetch,
  repoSetRerere,
  repoSetUntrackedCache,
  type RepoConfig,
} from '../ipc/commands';
import { messageOf } from '../ipc/error';
import type { AutoFetch } from '../ipc/types';

/**
 * The open repository's own git switches, as the Repository pane shows them.
 *
 * Read when the page opens and after every change, never assumed: each one is a line in the
 * repository's config that a terminal beside Coral can change just as well.
 */
export class RepoConfigState {
  config = $state<RepoConfig | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  #path = '';

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      this.config = await repoConfig(path);
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async setRerere(on: boolean): Promise<void> {
    await this.#change(() => repoSetRerere(this.#path, on));
  }

  async setUntrackedCache(on: boolean): Promise<void> {
    await this.#change(() => repoSetUntrackedCache(this.#path, on));
  }

  async setFsmonitor(on: boolean): Promise<void> {
    await this.#change(() => repoSetFsmonitor(this.#path, on));
  }

  async setSparse(folders: string[]): Promise<void> {
    await this.#change(() => repoSetSparse(this.#path, folders));
  }

  async setTracker(url: string): Promise<void> {
    await this.#change(() => repoSetTracker(this.#path, url.trim() === '' ? null : url.trim()));
  }

  async setAutoFetch(to: AutoFetch | null): Promise<void> {
    await this.#change(() => repoSetAutoFetch(this.#path, to));
  }

  async disableSparse(): Promise<void> {
    await this.#change(() => repoDisableSparse(this.#path));
  }

  /** The repository this describes, for the picker to read folders from. */
  get path(): string {
    return this.#path;
  }

  async #change(write: () => Promise<RepoConfig>): Promise<void> {
    if (this.#path === '') return;
    this.busy = true;
    this.error = null;
    try {
      this.config = await write();
    } catch (e) {
      this.error = messageOf(e);
    } finally {
      this.busy = false;
    }
  }
}
