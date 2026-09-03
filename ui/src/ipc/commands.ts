import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { GitRef, Submodule, RepoInfo } from './types';

/**
 * The only module that calls `invoke`. Types come from `types.ts`, which Rust generates via
 * ts-rs; the wrappers are hand-written because the binary graph path cannot be described by
 * any generator.
 */
export async function open(path: string): Promise<RepoInfo> {
  return invoke<RepoInfo>('open_repo', { path });
}

/** Which repository to open on launch. Replaced by the tab session later in M5. */
export async function initialRepo(): Promise<string> {
  return invoke<string>('initial_repo');
}

/**
 * Asks for a repository directory.
 *
 * A native picker rather than a text box: a path typed by hand is the one input nobody gets
 * right, and the dialog also confirms the directory exists before we try to open it.
 */
export async function pickRepository(): Promise<string | null> {
  const chosen = await openDialog({
    directory: true,
    multiple: false,
    title: 'Open a repository',
  });
  return typeof chosen === 'string' ? chosen : null;
}

/** A ref together with the graph row it labels, or null when that commit is not in the walk. */
export interface PlacedRef extends GitRef {
  row: number | null;
}

/** Every ref, each already resolved to the graph row it labels. */
export function repoRefs(path: string): Promise<PlacedRef[]> {
  return invoke<PlacedRef[]>('repo_refs', { path });
}

/** The repository's submodules. Empty for a repository that declares none. */
export function repoSubmodules(path: string): Promise<Submodule[]> {
  return invoke<Submodule[]>('repo_submodules', { path });
}
