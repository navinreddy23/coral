// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import { HostingState } from '../src/state/hosting.svelte';
import type { PullRequest } from '../src/ipc/commands';

function proposal(number: number, state: PullRequest['state']): PullRequest {
  return {
    number,
    title: `P${number}`,
    state,
    author: 'a',
    sourceBranch: `t${number}`,
    targetBranch: 'main',
    webUrl: `https://github.com/o/r/pull/${number}`,
    updatedAt: '2026-01-01T00:00:00Z',
    headSha: String(number % 10).repeat(40),
  };
}

// A block, not an expression: a function returned from beforeEach is run as a cleanup hook,
// and mockReset returns the mock.
beforeEach(() => {
  invoke.mockReset();
});

describe('reading the checks on proposals', () => {
  it('asks only for open ones, never more than four at a time, and keeps what comes back', async () => {
    let out = 0;
    let most = 0;
    invoke.mockImplementation(async (_cmd: string, args: { sha: string }) => {
      out += 1;
      most = Math.max(most, out);
      await new Promise((r) => setTimeout(r, 5));
      out -= 1;
      return { state: 'passing', summary: `ok ${args.sha.slice(0, 1)}`, url: null };
    });
    const hosting = new HostingState();
    const open = Array.from({ length: 9 }, (_, i) => proposal(i + 1, i % 2 === 0 ? 'open' : 'draft'));
    await hosting.readChecks('', [...open, proposal(20, 'merged'), proposal(21, 'closed')]);

    expect(invoke).toHaveBeenCalledTimes(9);
    expect(most).toBeLessThanOrEqual(4);
    expect(Object.keys(hosting.checks).map(Number).sort((a, b) => a - b)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
  });

  it('leaves out a proposal whose checks could not be read', async () => {
    invoke.mockImplementation(async (_cmd: string, args: { sha: string }) => {
      if (args.sha.startsWith('2')) throw { code: 'refused', message: 'rate limited' };
      return { state: 'failing', summary: '1 of 1 check failed', url: 'https://ci' };
    });
    const hosting = new HostingState();
    await hosting.readChecks('', [proposal(1, 'open'), proposal(2, 'open')]);
    expect(hosting.checks[1]?.state).toBe('failing');
    expect(hosting.checks[2]).toBeUndefined();
  });

  it('keeps the newer reading when an older one answers last', async () => {
    // A push moves a proposal's tip and the list is read again while the first reading is
    // still out; its answer is about the old commit.
    const old = proposal(1, 'open');
    const moved = { ...old, headSha: '9'.repeat(40) };
    const waiting = new Map<string, (v: unknown) => void>();
    invoke.mockImplementation(
      (_cmd: string, args: { sha: string }) => new Promise((r) => waiting.set(args.sha, r)),
    );
    const hosting = new HostingState();
    const first = hosting.readChecks('', [old]);
    const second = hosting.readChecks('', [moved]);
    waiting.get(moved.headSha)?.({ state: 'passing', summary: 'new', url: null });
    await second;
    waiting.get(old.headSha)?.({ state: 'failing', summary: 'old', url: null });
    await first;
    expect(hosting.checks[1]?.summary).toBe('new');
  });
});
