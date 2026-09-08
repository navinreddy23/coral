import type { MenuItem } from './Menu.svelte';
import { withoutRemote } from './refname';
import type { PlacedRef } from '../ipc/commands';
import type { Ancestry } from '../ipc/types';

/**
 * What can be done with the revision on a row, as menu lines.
 *
 * The wording and the shape of these lines is the whole of the decision — which of them are
 * offered, which way round they read, and which are there but refused. What each one actually
 * runs is the window's business, and arrives as `on`.
 */

/** What the window does when one of these lines is chosen. */
export interface RevisionActions {
  /** True while something is already running, which disables every line that acts. */
  busy: boolean;
  /** Goes to a branch, asking first where a remote and a local share the name. */
  goTo: (ref: PlacedRef) => void;
  /** Checks a revision out by name, which for a tag detaches HEAD. */
  checkout: (rev: string) => void;
  merge: (rev: string, ffOnly: boolean) => void;
  rebase: (onto: string) => void;
  rebaseInteractively: (onto: string) => void;
  /** Moves a branch forward without checking it out. */
  fastForwardBranch: (name: string, at: string) => void;
  /** Moves a tag, after saying what that costs anyone who already has it. */
  moveTag: (name: string, to: string) => void;
}

/**
 * Checking out what is on a row, by name.
 *
 * A local branch is checked out as itself. A tracking branch is checked out under its own name
 * without the remote in front, which is git's own rule and makes a local branch that follows
 * it. A tag has no branch to be on, so git detaches, and the menu says so rather than leaving
 * the user to discover it.
 */
export function checkoutItems(
  here: readonly PlacedRef[],
  head: string | null,
  on: RevisionActions,
): MenuItem[] {
  const alongside = new Set(here.filter((r) => r.kind.kind === 'local_branch').map((r) => r.short));
  const out: MenuItem[] = [];

  for (const ref of here) {
    if (ref.kind.kind === 'local_branch') {
      if (ref.short === head) continue;
      out.push({ kind: 'item', label: `Checkout ${ref.short}`, run: () => on.goTo(ref) });
    } else if (ref.kind.kind === 'remote_branch') {
      const name = withoutRemote(ref.short);
      // Not when the local branch of that name is on this very row and already offered above:
      // two entries that read the same and do the same is a menu nobody can answer. A local of
      // that name sitting elsewhere is a different matter — that entry is how the two get
      // reconciled, and `goTo` asks which way.
      if (name === '' || name === head || alongside.has(name)) continue;
      out.push({
        kind: 'item',
        label: `Checkout ${name}`,
        hint: `tracking ${ref.short}`,
        run: () => on.goTo(ref),
      });
    } else if (ref.kind.kind === 'tag') {
      out.push({
        kind: 'item',
        label: `Checkout ${ref.short}`,
        hint: 'detaches HEAD',
        run: () => on.checkout(ref.short),
      });
    }
  }
  return out;
}

/**
 * Bringing a revision into the current branch, and taking it along when the branch is in front.
 *
 * Every line is always here; only the fast-forward changes direction. Merging or rebasing onto
 * something the branch already contains is a no-op git states plainly, which is a better answer
 * than an item that is not there — and `rebase -i` onto an ancestor is not a no-op at all: it
 * lists every commit made since, which is how anybody edits the history since their last
 * release.
 */
export function combineItems(
  ref: PlacedRef | null,
  rev: string,
  head: string,
  where: Ancestry,
  on: RevisionActions,
): MenuItem[] {
  // The row the branch is already on. Everything here would be about itself.
  if (where === 'same') return [];
  return [
    fastForwardItem(ref, rev, head, where, on),
    {
      kind: 'item',
      label: `Merge ${rev} into ${head}`,
      disabled: on.busy,
      run: () => on.merge(rev, false),
    },
    {
      kind: 'item',
      label: `Rebase ${head} onto ${rev}`,
      disabled: on.busy,
      run: () => on.rebase(rev),
    },
    {
      kind: 'item',
      label: `Rebase ${head} onto ${rev}, interactively`,
      disabled: on.busy,
      run: () => on.rebaseInteractively(rev),
    },
  ];
}

/**
 * The fast-forward line, pointing whichever way git could actually take it.
 *
 * One line, always in the same place, because a menu whose items come and go is a menu nobody
 * can learn. What changes is the direction and whether it can be used. A ref the branch is
 * behind is fast-forwarded to; a ref the branch has passed is fast-forwarded *from*, which
 * moves it — a branch by fast-forward with no checkout, a tag by replacement, which is asked
 * about first because whoever has fetched the old one keeps it. A bare commit behind, or two
 * that have diverged, can be neither, and the line says why rather than vanishing: it read
 * "Fast-forward master to v1.0.0" on an up-to-date master, which git refuses because master is
 * the one in front.
 */
function fastForwardItem(
  ref: PlacedRef | null,
  rev: string,
  head: string,
  where: Ancestry,
  on: RevisionActions,
): MenuItem {
  if (where === 'ahead') {
    return {
      kind: 'item',
      label: `Fast-forward ${head} to ${rev}`,
      hint: 'never a merge commit',
      disabled: on.busy,
      run: () => on.merge(rev, true),
    };
  }
  if (where === 'behind' && ref?.kind.kind === 'local_branch') {
    return {
      kind: 'item',
      label: `Fast-forward ${ref.short} to ${head}`,
      hint: 'without checking it out',
      disabled: on.busy,
      run: () => on.fastForwardBranch(ref.short, head),
    };
  }
  if (where === 'behind' && ref?.kind.kind === 'tag') {
    return {
      kind: 'item',
      // Named for the direction rather than for the plumbing: git moves a tag by replacing it,
      // and the hint says so.
      label: `Fast-forward ${ref.short} to ${head}…`,
      hint: 'replaces the tag',
      danger: true,
      disabled: on.busy,
      run: () => on.moveTag(ref.short, head),
    };
  }
  return {
    kind: 'item',
    label: `Fast-forward ${head} to ${rev}`,
    hint: where === 'behind' ? `${head} is already past it` : 'they have diverged',
    disabled: true,
    run: () => {},
  };
}
