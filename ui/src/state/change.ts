import type { RepoChanged } from '../ipc/watch';

/**
 * What the watcher said while the window was busy, to read once it is free.
 *
 * The files, the index and an operation in progress only. A ref or a walk that moved while the
 * window was busy is almost always its own action's doing, which reloads them itself, and a
 * second walk after every commit is seconds on a kernel-sized repository.
 */
export function heldFor(later: RepoChanged | null, change: RepoChanged): RepoChanged | null {
  const files = change.index || change.worktree || change.ops;
  if (!files) return later;
  return {
    refs: false,
    graph: false,
    index: change.index || (later?.index ?? false),
    worktree: change.worktree || (later?.worktree ?? false),
    ops: change.ops || (later?.ops ?? false),
  };
}
