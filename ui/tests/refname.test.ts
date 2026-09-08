import { describe, expect, it } from 'vitest';

import { checkoutOf, divergence, remoteOf, withoutRemote } from '../src/app/refname';
import type { PlacedRef } from '../src/ipc/commands';

function ref(short: string, over: Partial<PlacedRef> = {}): PlacedRef {
  return {
    name: `refs/heads/${short}`,
    short,
    kind: { kind: 'local_branch' },
    target: 'a'.repeat(40),
    peeled: null,
    upstream: null,
    ahead: 0,
    behind: 0,
    row: 0,
    ...over,
  };
}

const tracking = (short: string) =>
  ref(short, { name: `refs/remotes/${short}`, kind: { kind: 'remote_branch', remote: 'origin' } });

describe('taking a tracking name apart', () => {
  it('splits it at the first slash, and only the first', () => {
    // Branch names carry slashes of their own: `origin/feature/lanes` is `feature/lanes` on
    // `origin`, not `lanes` on `origin/feature`.
    expect(remoteOf('origin/feature/lanes')).toBe('origin');
    expect(withoutRemote('origin/feature/lanes')).toBe('feature/lanes');
  });

  it('leaves a name with no remote in it alone', () => {
    expect(remoteOf('master')).toBe('');
    expect(withoutRemote('master')).toBe('master');
  });
});

describe('what checking a ref out is called', () => {
  it('drops the remote from a tracking branch, and says what it will track', () => {
    expect(checkoutOf(tracking('origin/topic'))).toEqual(['Checkout topic', 'tracking origin/topic']);
  });

  it('warns that a tag has nowhere to put HEAD', () => {
    expect(checkoutOf(ref('v1.0.0', { kind: { kind: 'tag', annotated: false } }))).toEqual([
      'Checkout v1.0.0',
      'detaches HEAD',
    ]);
  });

  it('says nothing extra about a local branch, because there is nothing to say', () => {
    expect(checkoutOf(ref('master'))).toEqual(['Checkout master', undefined]);
  });
});

describe('what to say about a local branch and the remote of its name', () => {
  it('offers to bring one that is merely behind up to date', () => {
    const said = divergence(ref('topic', { upstream: 'origin/topic', behind: 3 }), tracking('origin/topic'));
    expect(said).toContain('3 commits behind');
    expect(said).toContain('nothing of its own');
    // The one thing a reset takes with it that the branch itself does not hold.
    expect(said).toContain('uncommitted changes');
  });

  it('counts one commit as one commit', () => {
    const said = divergence(ref('topic', { upstream: 'origin/topic', behind: 1 }), tracking('origin/topic'));
    expect(said).toContain('1 commit behind');
  });

  it('warns that a branch with commits of its own would lose their name', () => {
    const said = divergence(ref('topic', { upstream: 'origin/topic', ahead: 2, behind: 3 }), tracking('origin/topic'));
    expect(said).toContain('different commits');
    expect(said).toContain('no name on it');
  });

  it('says the same for a branch that tracks nothing but shares the name', () => {
    // The remote branch is not its upstream, so nothing is known about how far apart they are.
    const said = divergence(ref('topic'), tracking('origin/topic'));
    expect(said).toContain('different commits');
  });
});
