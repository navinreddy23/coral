import { describe, expect, it } from 'vitest';

import { orderRefs, pillChars } from '../src/app/pill';
import type { PlacedRef } from '../src/ipc/commands';

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

const nothing = () => false;

describe('which labels a row shows', () => {
  it('puts the branch you are on first, whatever order they arrived in', () => {
    const order = orderRefs([tag('v1.0.0'), remote('origin/master'), local('master')], 'master', nothing);
    expect(order.map((r) => r.short)).toEqual(['master', 'v1.0.0', 'origin/master']);
  });

  it('cuts the tracking branch before the local one it repeats', () => {
    // A branch beside its own tracking branch is the commonest pair on a row, and there is
    // room for two: the one that says least has to be the one that goes.
    const order = orderRefs([remote('origin/topic'), local('topic')], null, nothing);
    expect(order.slice(0, 1).map((r) => r.short)).toEqual(['topic']);
  });

  it('ranks a tag above a tracking branch and below a local one', () => {
    const order = orderRefs([remote('origin/x'), tag('v2'), local('x')], null, nothing);
    expect(order.map((r) => r.short)).toEqual(['x', 'v2', 'origin/x']);
  });

  it('draws no label at all for a ref the graph is not walking', () => {
    // Its commits are often still on screen, because a branch that is walked reaches them.
    // Leaving the label on one put the name of a branch just hidden back on the graph, with
    // the struck eye in the panel beside it saying the opposite.
    const hidden = (name: string) => name === 'refs/heads/gone';
    const order = orderRefs([local('gone'), local('here')], null, hidden);
    expect(order.map((r) => r.short)).toEqual(['here']);
  });

  it('leaves the list it was given alone', () => {
    const labels = [remote('origin/x'), local('x')];
    orderRefs(labels, null, nothing);
    expect(labels.map((r) => r.short)).toEqual(['origin/x', 'x']);
  });
});

describe('how much of a name fits', () => {
  it('shows more of it as the column is dragged wider', () => {
    expect(pillChars(300)).toBeGreaterThan(pillChars(200));
  });

  it('never asks for fewer characters than make a name worth drawing', () => {
    // The column has a minimum, but the arithmetic runs negative before it: a pill with room
    // for two letters is a pill saying nothing.
    expect(pillChars(0)).toBe(10);
    expect(pillChars(62)).toBe(10);
  });
});
