import { describe, expect, it, vi } from 'vitest';

import { checkoutItems, combineItems, resetItem, type RevisionActions } from '../src/app/revision';
import type { PlacedRef } from '../src/ipc/commands';
import type { Ancestry } from '../src/ipc/types';

function ref(short: string, kind: PlacedRef['kind']): PlacedRef {
  return {
    name: kind.kind === 'remote_branch' ? `refs/remotes/${short}` : `refs/heads/${short}`,
    short,
    kind,
    target: 'a'.repeat(40),
    peeled: null,
    upstream: null,
    ahead: 0,
    behind: 0,
    row: 0,
  };
}

const local = (short: string) => ref(short, { kind: 'local_branch' });
const remote = (short: string) => ref(short, { kind: 'remote_branch', remote: 'origin' });
const tag = (short: string) => ref(short, { kind: 'tag', annotated: false });

function spies(busy = false) {
  return {
    busy,
    goTo: vi.fn(),
    checkout: vi.fn(),
    merge: vi.fn(),
    rebase: vi.fn(),
    rebaseInteractively: vi.fn(),
    fastForwardBranch: vi.fn(),
    moveTag: vi.fn(),
  } satisfies RevisionActions;
}

/** What a menu reads, top to bottom. */
function labels(items: ReturnType<typeof combineItems>): string[] {
  return items.map((i) => (i.kind === 'item' ? i.label : '—'));
}

function press(items: ReturnType<typeof combineItems>, label: string): void {
  const found = items.find((i) => i.kind === 'item' && i.label === label);
  if (found?.kind !== 'item') throw new Error(`no item called ${label}`);
  found.run();
}

describe('checking out what is on a row', () => {
  it('offers a tracking branch under its own name, and says what it will follow', () => {
    const on = spies();
    const items = checkoutItems([remote('origin/topic')], 'master', on);
    expect(labels(items)).toEqual(['Checkout topic']);
    expect(items[0]?.kind === 'item' && items[0].hint).toBe('tracking origin/topic');
  });

  it('offers the local branch alone when both are on the row', () => {
    // Two entries reading the same and doing the same is a menu nobody can answer.
    const items = checkoutItems([local('topic'), remote('origin/topic')], 'master', spies());
    expect(labels(items)).toEqual(['Checkout topic']);
  });

  it('still offers the remote when the local of that name is somewhere else', () => {
    // That entry is how the two get reconciled, and going there asks which way round.
    const items = checkoutItems([remote('origin/topic')], 'master', spies());
    expect(labels(items)).toEqual(['Checkout topic']);
  });

  it('does not offer to go where HEAD already is', () => {
    expect(checkoutItems([local('master')], 'master', spies())).toEqual([]);
    expect(checkoutItems([remote('origin/master')], 'master', spies())).toEqual([]);
  });

  it('warns that a tag has nowhere to put HEAD', () => {
    const items = checkoutItems([tag('v1.0.0')], 'master', spies());
    expect(items[0]?.kind === 'item' && items[0].hint).toBe('detaches HEAD');
  });

  it('asks the window which way round, for a branch, and checks a tag out by name', () => {
    const on = spies();
    const items = checkoutItems([local('topic'), tag('v1')], 'master', on);
    press(items, 'Checkout topic');
    press(items, 'Checkout v1');
    expect(on.goTo).toHaveBeenCalledTimes(1);
    expect(on.checkout.mock.calls).toEqual([['v1']]);
  });
});

describe('bringing a revision into the branch', () => {
  const four = ['Fast-forward', 'Merge', 'Rebase', 'Rebase'];

  it('offers the same four lines in the same order, whichever way they point', () => {
    for (const where of ['ahead', 'behind', 'diverged'] as Ancestry[]) {
      const items = combineItems(local('topic'), 'topic', 'master', where, spies());
      expect(labels(items).map((l) => l.split(' ')[0]), where).toEqual(four);
    }
  });

  it('says nothing about the row the branch is already on', () => {
    expect(combineItems(local('master'), 'master', 'master', 'same', spies())).toEqual([]);
  });

  it('fast-forwards the branch to a revision it is behind', () => {
    const on = spies();
    const items = combineItems(local('topic'), 'topic', 'master', 'ahead', on);
    press(items, 'Fast-forward master to topic');
    expect(on.merge.mock.calls, 'a merge git will only take as a fast-forward').toEqual([
      ['topic', true],
    ]);
  });

  it('fast-forwards a branch the other way, without checking it out', () => {
    const on = spies();
    const items = combineItems(local('topic'), 'topic', 'master', 'behind', on);
    press(items, 'Fast-forward topic to master');
    expect(on.fastForwardBranch.mock.calls).toEqual([['topic', 'master']]);
  });

  it('moves a tag rather than fast-forwarding it, and marks that as destructive', () => {
    // Git moves a tag by replacing it, and whoever has fetched the old one keeps it.
    const on = spies();
    const items = combineItems(tag('v1.0.0'), 'v1.0.0', 'master', 'behind', on);
    const line = items.find((i) => i.kind === 'item' && i.label.startsWith('Fast-forward v1.0.0'));
    expect(line?.kind === 'item' && line.danger).toBe(true);
    press(items, 'Fast-forward v1.0.0 to master…');
    expect(on.moveTag.mock.calls).toEqual([['v1.0.0', 'master']]);
  });

  it('keeps the line but refuses it where git would, and says why', () => {
    // It read "Fast-forward master to v1.0.0" on an up-to-date master, which git refuses
    // because master is the one in front. A menu whose items come and go cannot be learnt, so
    // the line stays and explains itself.
    const behind = combineItems(null, 'a1b2c3d4', 'master', 'behind', spies());
    const first = behind[0];
    expect(first?.kind === 'item' && first.disabled).toBe(true);
    // Without the branch name, which the label beside it carries: with it, the reason was
    // elided out of the row on any branch not called "master".
    expect(first?.kind === 'item' && first.hint).toBe('already past it');

    const apart = combineItems(null, 'a1b2c3d4', 'master', 'diverged', spies());
    expect(apart[0]?.kind === 'item' && apart[0].hint).toBe('they have diverged');
  });

  it('disables every line that acts while something is already running', () => {
    const items = combineItems(local('topic'), 'topic', 'master', 'ahead', spies(true));
    expect(items.every((i) => i.kind === 'item' && i.disabled)).toBe(true);
  });

  it('offers an interactive rebase onto an ancestor, which is not a no-op', () => {
    // It lists every commit made since, which is how anybody edits their history since a
    // release.
    const on = spies();
    const items = combineItems(tag('v1.0.0'), 'v1.0.0', 'master', 'behind', on);
    press(items, 'Rebase master onto v1.0.0, interactively');
    expect(on.rebaseInteractively.mock.calls).toEqual([['v1.0.0']]);
  });
});

describe('resetting a branch to a commit', () => {
  /**
   * The same words did two different things. Right-clicking a commit row opened a submenu and
   * asked which reset; right-clicking the tag or branch label *on that row* ran a hard one
   * behind a confirmation. A reader who learned the first meaning met the second by surprise.
   */
  it('always asks which reset, wherever it is opened from', () => {
    const soft = vi.fn();
    const mixed = vi.fn();
    const hard = vi.fn();
    const item = resetItem('main', { soft, mixed, hard }, false);

    expect(item.kind).toBe('submenu');
    if (item.kind !== 'submenu') throw new Error('a submenu');
    expect(item.label).toBe('Reset main to this commit');
    expect(labels(item.items)).toEqual([
      'Soft — keep the index and the working copy',
      'Mixed — keep the working copy',
      'Hard — discard everything since',
    ]);

    press(item.items, 'Soft — keep the index and the working copy');
    press(item.items, 'Mixed — keep the working copy');
    press(item.items, 'Hard — discard everything since');
    expect([soft.mock.calls.length, mixed.mock.calls.length, hard.mock.calls.length]).toEqual([
      1, 1, 1,
    ]);
  });

  it('marks only the one that throws work away', () => {
    const item = resetItem('main', { soft: vi.fn(), mixed: vi.fn(), hard: vi.fn() }, false);
    if (item.kind !== 'submenu') throw new Error('a submenu');
    const danger = item.items.map((i) => (i.kind === 'item' ? i.danger === true : false));
    expect(danger).toEqual([false, false, true]);
  });

  it('refuses every mode while something else is running', () => {
    const item = resetItem('main', { soft: vi.fn(), mixed: vi.fn(), hard: vi.fn() }, true);
    if (item.kind !== 'submenu') throw new Error('a submenu');
    expect(item.items.every((i) => i.kind === 'item' && i.disabled === true)).toBe(true);
  });
});
