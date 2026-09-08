import { remoteEdit, remoteList, type RemoteEdit } from '../ipc/commands';
import type { Remote } from '../ipc/types';
import { messageOf } from '../ipc/error';

/**
 * The repository's remotes.
 *
 * Every edit answers with the whole list, so nothing here applies a change locally and then
 * hopes it matches what git did.
 */
export class RemotesState {
  list = $state<Remote[]>([]);
  busy = $state(false);
  error = $state<string | null>(null);
  /**
   * Whether the list has been read at all.
   *
   * An empty list means two different things before and after that, and the panel has to tell
   * them apart: it opens on the add form when a repository has no remotes, and opened on it in
   * repositories that had two, because it decided before the answer arrived.
   */
  read = $state(false);
  #path = '';

  async load(path: string): Promise<void> {
    this.#path = path;
    this.read = false;
    await this.#run(() => remoteList(path));
    this.read = true;
  }

  async edit(edit: RemoteEdit): Promise<boolean> {
    if (!this.#path) return false;
    const path = this.#path;
    return this.#run(() => remoteEdit(path, edit));
  }

  /** True when the call succeeded, so a dialog knows whether to close. */
  async #run(action: () => Promise<Remote[]>): Promise<boolean> {
    this.busy = true;
    this.error = null;
    try {
      this.list = await action();
      return true;
    } catch (e) {
      this.error = messageOf(e);
      return false;
    } finally {
      this.busy = false;
    }
  }
}
