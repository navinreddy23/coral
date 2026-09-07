import {
  sshGenerate,
  sshKeys,
  sshPublicKey,
  sshRead,
  sshSetApp,
  sshSetRepo,
} from '../ipc/commands';
import type { SshConfig, SshKey, SshOverrides, SshScopes } from '../ipc/types';
import { messageOf } from '../ipc/error';

/** Which level the settings screen is editing. */
export type Level = 'app' | 'repo';

/**
 * How git reaches an ssh server, for the open repository.
 *
 * Two levels, for the same reason signing has two: a key belongs to a repository rather than
 * to a person, and the same user pushes to work with a company key and to their own account
 * with another. The app level is the default and a repository overrides what it needs to.
 */
export class SshState {
  scopes = $state<SshScopes | null>(null);
  keys = $state<SshKey[]>([]);
  level = $state<Level>('repo');
  busy = $state(false);
  error = $state<string | null>(null);
  /** What the last successful action did, so the screen can confirm it. */
  done = $state<string | null>(null);

  #path = '';

  /** What git will actually do here. */
  effective = $derived<SshConfig | null>(this.scopes?.effective ?? null);

  /** True when this repository sets nothing and simply follows the app level. */
  inheriting = $derived.by(() => {
    const local = this.scopes?.local;
    if (!local) return true;
    return (
      local.privateKey === null &&
      local.publicKey === null &&
      local.credentialHelper === null
    );
  });

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      this.scopes = await sshRead(path);
    } catch (e) {
      this.error = messageOf(e);
    }
    await this.loadKeys();
  }

  /**
   * The keys on this machine, which do not belong to any repository.
   *
   * Separate from [`load`](#load) because the clone form needs them before there is a
   * repository to read settings from.
   */
  async loadKeys(): Promise<void> {
    try {
      this.keys = await sshKeys();
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async saveApp(config: SshConfig): Promise<void> {
    await this.#run(() => sshSetApp(this.#path, config), 'App-level ssh settings saved');
  }

  async saveRepo(overrides: SshOverrides): Promise<void> {
    await this.#run(
      () => sshSetRepo(this.#path, overrides),
      'This repository’s ssh settings saved',
    );
  }

  /** Clears every override, so the repository follows the app level again. */
  async inherit(): Promise<void> {
    await this.saveRepo({ privateKey: null, publicKey: null, credentialHelper: null });
  }

  /** Creates a key pair and lists it. Does not select it: that is a separate decision. */
  async generate(name: string, comment: string, passphrase: string): Promise<SshKey | null> {
    this.busy = true;
    this.error = null;
    this.done = null;
    try {
      const key = await sshGenerate(name, comment, passphrase);
      this.keys = await sshKeys();
      this.done = `Created ${key.path}`;
      return key;
    } catch (e) {
      this.error = messageOf(e);
      return null;
    } finally {
      this.busy = false;
    }
  }

  /** The public key's own text, which is what a host asks to be pasted in. */
  async readPublic(path: string): Promise<string> {
    try {
      return await sshPublicKey(path);
    } catch (e) {
      this.error = messageOf(e);
      return '';
    }
  }

  async #run(action: () => Promise<SshScopes>, said: string): Promise<void> {
    this.busy = true;
    this.error = null;
    this.done = null;
    try {
      this.scopes = await action();
      this.done = said;
    } catch (e) {
      this.error = messageOf(e);
    } finally {
      this.busy = false;
    }
  }
}
