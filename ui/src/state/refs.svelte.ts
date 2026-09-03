import { repoRefs, repoSubmodules, type PlacedRef } from '../ipc/commands';
import type { Submodule } from '../ipc/types';

export type { PlacedRef };

/** Refs grouped the way the sidebar shows them. */
export interface RefGroups {
  local: PlacedRef[];
  remote: PlacedRef[];
  tags: PlacedRef[];
  stashes: PlacedRef[];
}

/**
 * The repository's refs, and which row each labels.
 *
 * Rows are resolved in Rust, where the store already holds a sorted index; doing it here would
 * mean shipping every object id to JavaScript to build a map.
 */
export class RefsState {
  all = $state<PlacedRef[]>([]);
  submodules = $state<Submodule[]>([]);
  error = $state<string | null>(null);

  groups = $derived<RefGroups>({
    local: this.all.filter((r) => r.kind.kind === 'local_branch'),
    remote: this.all.filter((r) => r.kind.kind === 'remote_branch'),
    tags: this.all.filter((r) => r.kind.kind === 'tag'),
    stashes: this.all.filter((r) => r.kind.kind === 'stash'),
  });

  /** Labels for each row, so the graph can look them up without scanning every ref. */
  byRow = $derived.by(() => {
    const map = new Map<number, PlacedRef[]>();
    for (const r of this.all) {
      if (r.row === null) continue;
      const at = map.get(r.row);
      if (at) at.push(r);
      else map.set(r.row, [r]);
    }
    return map;
  });

  async load(path: string): Promise<void> {
    this.error = null;
    try {
      this.all = await repoRefs(path);
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.all = [];
    }
    // Submodules are a separate read and a separate failure: a repository whose .gitmodules
    // is unreadable should still show its branches.
    try {
      this.submodules = await repoSubmodules(path);
    } catch {
      this.submodules = [];
    }
  }
}
