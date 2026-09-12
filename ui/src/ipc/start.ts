import { invoke } from './invoke';
import type { CloneOutcome } from './types';

/** One repository this user has opened before. */
export interface Recent {
  path: string;
  /** The last path segment, which is what people call a repository. */
  name: string;
  /** Seconds since the epoch. */
  opened: number;
  /** Whether the repository has gone from where it was. */
  missing: boolean;
}

export function recentRepos(): Promise<Recent[]> {
  return invoke<Recent[]>('recent_repos');
}

/** Takes one off the list. The repository itself is not touched. */
export function forgetRecent(path: string): Promise<Recent[]> {
  return invoke<Recent[]>('forget_recent', { path });
}

/** Empties the list. The repositories themselves are not touched. */
export function forgetAllRecents(): Promise<Recent[]> {
  return invoke<Recent[]>('forget_all_recents', {});
}

/**
 * Creates an empty repository and answers with where it is.
 *
 * `branch` names the first branch; empty leaves git's own default, which the user may have
 * configured and which Coral has no business overriding.
 */
export function repoInit(path: string, branch: string, lfs: boolean): Promise<string> {
  return invoke<string>('repo_init', { path, branch: branch.trim() || null, lfs });
}

/** How much of a repository to take. */
export type CloneHistory = 'full' | 'shallow' | 'blobless';

/** Everything a clone is asked for. */
export interface CloneWanted {
  url: string;
  /** The directory the clone is made in; the repository appears under it. */
  parent: string;
  /** Empty uses the name in the URL, as git does. */
  name: string;
  /** A private key path, or empty to leave it to the agent. */
  sshKey: string;
  history: CloneHistory;
  /** How many commits to take. Only read when `history` is shallow. */
  depth: number;
}

/**
 * Clones into `parent`, under `name` or under the name in the URL.
 *
 * `notes` is anything git said that was not progress. A clone can exit 0 and check nothing
 * out — "remote HEAD refers to nonexistent ref" — and this is how that reaches the window.
 */
export function repoClone(wanted: CloneWanted): Promise<CloneOutcome> {
  return invoke<CloneOutcome>('repo_clone', {
    request: {
      url: wanted.url,
      parent: wanted.parent,
      name: wanted.name.trim() || null,
      sshKey: wanted.sshKey.trim() || null,
      depth: wanted.history === 'shallow' ? Math.max(1, Math.round(wanted.depth)) : null,
      blobless: wanted.history === 'blobless',
    },
  });
}

/** Whether `git lfs` is on this machine, so the tick box is only offered when it can work. */
export function lfsAvailable(): Promise<boolean> {
  return invoke<boolean>('lfs_available');
}
