import { invoke } from './invoke';
import type { Identity, IdentityScopes, SigningOverrides, SshOverrides } from './types';
import type { GroupColour, Session } from '../state/tabs.svelte';
import type { Recent } from './start';

/**
 * Who is at the window, and the workspace that belongs to them.
 *
 * Hand-written rather than generated, as everything `coral-app` owns is: `ts-rs` covers the
 * engine's types in `coral-core`, not the command layer's. The three settings shapes below do
 * come from there, because applying a profile to a repository is the same write the settings
 * screen makes.
 */
export interface ProfileSettings {
  user: Identity;
  ssh: SshOverrides;
  signing: SigningOverrides;
}

export interface Profile {
  /** The name of its directory. Derived from the name, and never a path. */
  id: string;
  name: string;
  colour: GroupColour;
  settings: ProfileSettings;
}

export interface Registry {
  profiles: Profile[];
  current: string;
}

/** A registry and the workspace that came with it, answered together so the two cannot drift. */
export interface Switched {
  registry: Registry;
  session: Session;
  recents: Recent[];
}

export function profileList(): Promise<Registry> {
  return invoke<Registry>('profile_list');
}

export function profileCreate(name: string, colour: GroupColour): Promise<Registry> {
  return invoke<Registry>('profile_create', { name, colour });
}

export function profileRename(id: string, name: string): Promise<Registry> {
  return invoke<Registry>('profile_rename', { id, name });
}

export function profileRecolour(id: string, colour: GroupColour): Promise<Registry> {
  return invoke<Registry>('profile_recolour', { id, colour });
}

export function profileSetSettings(id: string, settings: ProfileSettings): Promise<Registry> {
  return invoke<Registry>('profile_set_settings', { id, settings });
}

export function profileSwitch(id: string): Promise<Switched> {
  return invoke<Switched>('profile_switch', { id });
}

export function profileDelete(id: string): Promise<Switched> {
  return invoke<Switched>('profile_delete', { id });
}

/** Writes the current profile's settings into one repository. */
export function profileApplyHere(path: string): Promise<IdentityScopes> {
  return invoke<IdentityScopes>('profile_apply_here', { path });
}

/** Who a repository records as the author of its commits, at every level. */
export function repoIdentity(path: string): Promise<IdentityScopes> {
  return invoke<IdentityScopes>('repo_identity', { path });
}
