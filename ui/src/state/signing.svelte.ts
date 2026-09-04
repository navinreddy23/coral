import {
  signingGenerate,
  signingKeys,
  signingRead,
  signingSetApp,
  signingSetRepo,
} from '../ipc/commands';
import { messageOf } from '../ipc/error';
import type {
  SigningConfig,
  SigningFormat,
  SigningKey,
  SigningOverrides,
  SigningScopes,
} from '../ipc/types';

/** Which level the settings screen is editing. */
export type Level = 'app' | 'repo';

/**
 * Commit signing for the open repository.
 *
 * Two levels, because a key belongs to a repository rather than to a person: the same user
 * signs work commits with a company key and their own with another. The app level is a
 * default every repository inherits, and a repository overrides what it needs to.
 */
export class SigningState {
  scopes = $state<SigningScopes | null>(null);
  keys = $state<SigningKey[]>([]);
  /** Which level the screen is editing. */
  level = $state<Level>('repo');
  loadingKeys = $state(false);
  busy = $state(false);
  error = $state<string | null>(null);
  /** What the last successful action did, so the screen can confirm it. */
  done = $state<string | null>(null);

  #path = '';

  /** What git will actually do here. */
  effective = $derived<SigningConfig | null>(this.scopes?.effective ?? null);

  /** True when this repository sets nothing and simply follows the app level. */
  inheriting = $derived.by(() => {
    const local = this.scopes?.local;
    if (!local) return true;
    return (
      local.format === null &&
      local.program === null &&
      local.key === null &&
      local.signCommits === null &&
      local.signTags === null
    );
  });

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      this.scopes = await signingRead(path);
      await this.refreshKeys();
    } catch (e) {
      this.error = message(e);
    }
  }

  /** Re-reads the keys the signing program can see. */
  async refreshKeys(): Promise<void> {
    const config = this.scopes?.effective;
    if (!config) return;
    this.loadingKeys = true;
    try {
      this.keys = await signingKeys(config.format, config.program);
    } catch (e) {
      // A missing or misnamed program is the usual cause, and it is worth saying so rather
      // than showing an empty list that looks like "you have no keys".
      this.error = message(e);
      this.keys = [];
    } finally {
      this.loadingKeys = false;
    }
  }

  /** Saves the app-level defaults. */
  async saveApp(config: SigningConfig): Promise<void> {
    await this.#run('Saved for every repository', () => signingSetApp(this.#path, config));
  }

  /** Saves what this repository overrides. A null field clears that override. */
  async saveRepo(overrides: SigningOverrides): Promise<void> {
    await this.#run('Saved for this repository', () => signingSetRepo(this.#path, overrides));
  }

  /** Drops every override so the repository follows the app level again. */
  async inherit(): Promise<void> {
    await this.saveRepo({
      format: null,
      program: null,
      key: null,
      signCommits: null,
      signTags: null,
    });
  }

  /** Creates a key and selects it at the level being edited. */
  async generate(program: string, passphrase: string): Promise<void> {
    this.busy = true;
    this.error = null;
    this.done = null;
    try {
      const key = await signingGenerate(this.#path, program, passphrase);
      await this.refreshKeys();
      const current = this.scopes?.effective;
      if (this.level === 'app' && current) {
        await signingSetApp(this.#path, { ...current, key: key.id });
      } else {
        await signingSetRepo(this.#path, {
          ...(this.scopes?.local ?? emptyOverrides()),
          key: key.id,
        });
      }
      this.scopes = await signingRead(this.#path);
      this.done = `Created ${key.label} and set it as the signing key`;
    } catch (e) {
      this.error = message(e);
    } finally {
      this.busy = false;
    }
  }

  async #run(what: string, action: () => Promise<SigningScopes>): Promise<void> {
    this.busy = true;
    this.error = null;
    this.done = null;
    try {
      this.scopes = await action();
      this.done = what;
    } catch (e) {
      this.error = message(e);
    } finally {
      this.busy = false;
    }
  }
}

function emptyOverrides(): SigningOverrides {
  return { format: null, program: null, key: null, signCommits: null, signTags: null };
}

function message(e: unknown): string {
  return messageOf(e);
}

/** The program git uses for a format when none is configured. */
export function defaultProgram(format: SigningFormat): string {
  return format === 'ssh' ? 'ssh-keygen' : format === 'x509' ? 'gpgsm' : 'gpg';
}
