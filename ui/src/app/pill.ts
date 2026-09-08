import type { PlacedRef } from '../ipc/commands';

/**
 * The labels drawn on a commit row.
 *
 * A row can carry a dozen refs and there is room for two, so which two is a decision with a
 * right answer rather than whatever order the walk produced.
 */

/**
 * Which of a row's refs are worth the slots there are.
 *
 * The checked-out branch first, then other local branches, then tags, then tracking branches:
 * the ones cut have to be the ones that say least. A tracking branch beside the local branch
 * it tracks is the commonest pair, and it is the tracking one that repeats what is there.
 */
export function orderRefs(
  labels: readonly PlacedRef[],
  head: string | null,
  /** True for a ref the graph is not being walked from. */
  hidden: (name: string) => boolean,
): PlacedRef[] {
  const rank = (r: PlacedRef): number => {
    if (r.short === head) return 0;
    switch (r.kind.kind) {
      case 'local_branch':
        return 1;
      case 'tag':
        return 2;
      case 'stash':
        return 3;
      default:
        return 4;
    }
  };
  // A hidden ref is not drawn at all. Its commits often stay, because a branch that is walked
  // still reaches them, and leaving the label on one of them put the name of a branch the user
  // had just hidden back on the graph — with the struck eye beside it in the panel saying the
  // opposite.
  return [...labels].filter((r) => !hidden(r.name)).sort((a, b) => rank(a) - rank(b));
}

/**
 * How many characters a name has room for in a ref column `columnPx` wide.
 *
 * Derived from the column the user has dragged rather than fixed: widening it should show more
 * of the name, which is the only reason to widen it. The constants are the pill's own
 * furniture — its icon, padding and border — and the width of a digit in the interface face.
 */
export function pillChars(columnPx: number): number {
  return Math.max(10, Math.floor((columnPx - 62) / 5.9));
}
