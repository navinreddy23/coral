import { invoke } from './invoke';
import type { ReflogEntry } from './types';

/** Where HEAD has been, newest first, with the commits nothing else reaches marked lost. */
export function repoReflog(path: string): Promise<ReflogEntry[]> {
  return invoke<ReflogEntry[]>('repo_reflog', { path });
}
