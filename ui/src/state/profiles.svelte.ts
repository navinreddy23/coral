/**
 * Who is at the window.
 *
 * Per profile rather than per person, which is why none of this is in `localStorage`: what
 * `views.svelte.ts` keeps there belongs to whoever is sitting at the window, and this is the
 * question of which of them that is. It travels with the workspace, in Rust.
 */

import {
  profileApplyHere,
  profileCreate,
  profileDelete,
  profileList,
  profileRecolour,
  profileRename,
  profileSetSettings,
  profileSwitch,
  type Profile,
  type ProfileSettings,
  type Registry,
  type Switched,
} from '../ipc/profiles';
import type { GroupColour } from './tabs.svelte';
import { messageOf } from '../ipc/error';

/** The profile a window with no answer yet is in, so nothing renders on nothing. */
const NOBODY: Profile = {
  id: '',
  name: 'Personal',
  colour: 'lane1',
  settings: {
    user: { name: null, email: null },
    ssh: { privateKey: null, publicKey: null, credentialHelper: null },
    signing: { format: null, program: null, key: null, signCommits: null, signTags: null },
  },
};

export class ProfilesState {
  all = $state<Profile[]>([NOBODY]);
  currentId = $state('');
  error = $state<string | null>(null);
  busy = $state(false);

  /**
   * The profile in use.
   *
   * Never absent: the chip in the title strip renders on it before the first answer arrives,
   * and a window that opened with no name on it would look broken rather than loading.
   */
  readonly current = $derived(
    this.all.find((p) => p.id === this.currentId) ?? this.all[0] ?? NOBODY,
  );

  async load(): Promise<void> {
    await this.#run(profileList);
  }

  async create(name: string, colour: GroupColour): Promise<void> {
    await this.#run(() => profileCreate(name, colour));
  }

  async rename(id: string, name: string): Promise<void> {
    await this.#run(() => profileRename(id, name));
  }

  async recolour(id: string, colour: GroupColour): Promise<void> {
    await this.#run(() => profileRecolour(id, colour));
  }

  async setSettings(id: string, settings: ProfileSettings): Promise<void> {
    await this.#run(() => profileSetSettings(id, settings));
  }

  /**
   * Changes profile, answering with the workspace that comes with it.
   *
   * The caller has to put the previous repository away before it uses the answer: the tabs it
   * carries are a different set, and the graph, the watcher and the settings page are all
   * still pointed at one of the tabs that has just closed.
   */
  async switchTo(id: string): Promise<Switched | null> {
    return this.#switching(() => profileSwitch(id));
  }

  /** Removes a profile. Its repositories are not touched, only the record of what was open. */
  async remove(id: string): Promise<Switched | null> {
    return this.#switching(() => profileDelete(id));
  }

  /** Writes this profile's identity, key and signing settings into one repository. */
  async applyHere(path: string): Promise<boolean> {
    this.error = null;
    try {
      await profileApplyHere(path);
      return true;
    } catch (e) {
      this.error = messageOf(e);
      return false;
    }
  }

  #take(registry: Registry): void {
    this.all = registry.profiles.length > 0 ? registry.profiles : [NOBODY];
    this.currentId = registry.current;
  }

  async #run(action: () => Promise<Registry>): Promise<void> {
    this.error = null;
    try {
      this.#take(await action());
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async #switching(action: () => Promise<Switched>): Promise<Switched | null> {
    this.busy = true;
    this.error = null;
    try {
      const answer = await action();
      this.#take(answer.registry);
      return answer;
    } catch (e) {
      this.error = messageOf(e);
      return null;
    } finally {
      this.busy = false;
    }
  }
}
