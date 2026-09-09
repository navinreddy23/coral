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

/**
 * The fewest characters worth drawing a name in.
 *
 * Below this the pill is clipped to an initial and an ellipsis — "m…", or, for a name whose
 * first characters are wide, "……" — which costs the same room as a name and says nothing.
 */
const WORTH_READING = 5;

/**
 * Whether a pill in a column this wide should carry its name at all.
 *
 * Squeezed narrow — the terminal docked beside the graph, with the detail panel open — the
 * branch column dropped to "m…", "……", "st…". The mark alone still says a branch or a tag is
 * here, the colour still says which, and the whole name is a hover away.
 */
export function pillNamed(columnPx: number): boolean {
  return Math.floor((columnPx - 62) / 5.9) >= WORTH_READING;
}
