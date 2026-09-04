import { invoke } from './invoke';
import type { StashEntry } from './types';

/**
 * A stash entry, with the row it is drawn on and the name to call it by.
 *
 * The name comes from the engine rather than being assembled here: git gives two stashes made
 * from the same commit the same subject, and deciding what to call them instead is a rule the
 * two front ends must not each have their own version of.
 */
export interface PlacedStash extends StashEntry {
  name: string;
  /** Null when the stash's commit is outside the loaded graph. */
  row: number | null;
}

/**
 * The stash stack, newest first.
 *
 * The stack, not the ref. `refs/stash` is the top of it and the only stash git keeps a ref for,
 * so listing refs finds one stash however many there are.
 */
export function repoStashes(path: string): Promise<PlacedStash[]> {
  return invoke<PlacedStash[]>('repo_stashes', { path });
}
