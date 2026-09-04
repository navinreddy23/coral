import { invoke } from './invoke';

/** One repository this user has opened before. */
export interface Recent {
  path: string;
  /** The last path segment, which is what people call a repository. */
  name: string;
  /** Seconds since the epoch. */
  opened: number;
}

export function recentRepos(): Promise<Recent[]> {
  return invoke<Recent[]>('recent_repos');
}

/** Takes one off the list. The repository itself is not touched. */
export function forgetRecent(path: string): Promise<Recent[]> {
  return invoke<Recent[]>('forget_recent', { path });
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

/** Clones into `parent`, under `name` or under the name in the URL. */
export function repoClone(url: string, parent: string, name: string): Promise<string> {
  return invoke<string>('repo_clone', { url, parent, name: name.trim() || null });
}

/** Whether `git lfs` is on this machine, so the tick box is only offered when it can work. */
export function lfsAvailable(): Promise<boolean> {
  return invoke<boolean>('lfs_available');
}
