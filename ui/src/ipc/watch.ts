import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { invoke, isPreview } from './invoke';
import type { RepoChanged } from './types';

export type { RepoChanged };

/** What starting a watch reports back. */
export interface Watching {
  /** False when the worktree could not be watched, so the window should refresh on focus. */
  complete: boolean;
  detail: string | null;
}

/**
 * Watches one repository, replacing whatever was being watched before.
 *
 * Everything Coral can do the terminal beside it can do too, and a client that only notices its
 * own writes shows the wrong branch until something else happens to reload.
 */
export function watchRepo(path: string): Promise<Watching> {
  return invoke<Watching>('watch_repo', { path });
}

export function unwatchRepo(): Promise<void> {
  return invoke<void>('unwatch_repo');
}

/**
 * Subscribes to change notifications.
 *
 * In the browser preview there is no repository and nothing to watch, so this subscribes to
 * nothing and hands back a no-op.
 */
export async function onRepoChanged(
  handle: (change: RepoChanged) => void,
): Promise<UnlistenFn> {
  if (isPreview()) return () => undefined;
  return listen<RepoChanged>('repo://changed', (event) => handle(event.payload));
}
