import type { PlacedRef } from '../ipc/commands';

/**
 * What to call a ref, and what to say about it.
 *
 * A tracking branch is written `origin/main` and is the branch `main` on the remote `origin`,
 * and almost everything the window says about one has to take that name apart first.
 */

/** The remote a tracking name belongs to, or the empty string for a name that has none. */
export function remoteOf(name: string): string {
  const at = name.indexOf('/');
  return at < 0 ? '' : name.slice(0, at);
}

/** The branch part of a tracking name, so `origin/main` and `main` can be compared. */
export function withoutRemote(name: string): string {
  const at = name.indexOf('/');
  return at < 0 ? name : name.slice(at + 1);
}

/** How one ref is checked out: what to call the menu item, and what it will do. */
export function checkoutOf(ref: PlacedRef): [string, string | undefined] {
  if (ref.kind.kind === 'remote_branch') {
    return [`Checkout ${withoutRemote(ref.short)}`, `tracking ${ref.short}`];
  }
  if (ref.kind.kind === 'tag') {
    return [`Checkout ${ref.short}`, 'detaches HEAD'];
  }
  return [`Checkout ${ref.short}`, undefined];
}

/**
 * What to tell someone before checking out a remote branch whose name a local branch has.
 *
 * `git checkout topic` for `origin/topic` lands on the existing local `topic`, wherever that
 * happens to be. The two cases read differently: a local branch that is merely behind can be
 * brought up to date with nothing lost, and one that has commits of its own cannot.
 */
export function divergence(local: PlacedRef, ref: PlacedRef): string {
  if (local.upstream === ref.short && local.ahead === 0 && local.behind > 0) {
    const many = local.behind === 1 ? 'commit' : 'commits';
    return (
      `${local.short} is ${local.behind} ${many} behind ${ref.short} and has nothing of its ` +
      'own. Resetting brings it up to date; uncommitted changes in the working copy are ' +
      'discarded with it.'
    );
  }
  return (
    `The local ${local.short} and ${ref.short} are on different commits. Checking out goes to ` +
    'the local branch as it stands. Resetting moves it onto the remote, and any commit only ' +
    'the local branch reached is left with no name on it.'
  );
}
