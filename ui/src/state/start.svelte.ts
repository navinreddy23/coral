import {
  forgetAllRecents,
  forgetRecent,
  lfsAvailable,
  recentRepos,
  repoClone,
  repoInit,
  type CloneWanted,
  type Recent,
} from '../ipc/start';
import { messageOf } from '../ipc/error';

/** Which form the start page is showing, if any. */
export type StartForm = 'none' | 'clone' | 'create';

/** The start page: what has been opened before, and what can be made. */
export class StartState {
  recents = $state<Recent[]>([]);
  filter = $state('');
  form = $state<StartForm>('none');
  busy = $state(false);
  error = $state<string | null>(null);
  /** Null until asked, so the tick box is not offered before the answer arrives. */
  lfs = $state<boolean | null>(null);

  /** The recents that match the filter, which is matched against the name and the path. */
  shown = $derived.by(() => {
    const want = this.filter.trim().toLowerCase();
    if (want === '') return this.recents;
    return this.recents.filter(
      (r) => r.name.toLowerCase().includes(want) || r.path.toLowerCase().includes(want),
    );
  });

  async load(): Promise<void> {
    this.error = null;
    try {
      this.recents = await recentRepos();
    } catch (e) {
      this.error = messageOf(e);
    }
    try {
      this.lfs = await lfsAvailable();
    } catch {
      // Not knowing means not offering, which is the safe way to be wrong about it.
      this.lfs = false;
    }
  }

  async forget(path: string): Promise<void> {
    try {
      this.recents = await forgetRecent(path);
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  /** Empties the list, for someone who does not want their repositories named on this page. */
  async forgetAll(): Promise<void> {
    try {
      this.recents = await forgetAllRecents();
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  /** Creates a repository and answers with where it is, or null when it could not be made. */
  async create(parent: string, name: string, branch: string, lfs: boolean): Promise<string | null> {
    const path = `${parent.replace(/\/+$/u, '')}/${name.trim()}`;
    return this.#run(() => repoInit(path, branch, lfs));
  }

  async clone(wanted: CloneWanted): Promise<string | null> {
    return this.#run(() => repoClone(wanted));
  }

  async #run(action: () => Promise<string>): Promise<string | null> {
    this.busy = true;
    this.error = null;
    try {
      return await action();
    } catch (e) {
      this.error = messageOf(e);
      return null;
    } finally {
      this.busy = false;
    }
  }
}
