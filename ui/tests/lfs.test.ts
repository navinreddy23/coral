import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls = vi.hoisted(() => ({
  locks: vi.fn(),
  lock: vi.fn(),
  unlock: vi.fn(),
  track: vi.fn(),
}));
vi.mock('../src/ipc/commands', () => ({
  lfsLocks: calls.locks,
  lfsLock: calls.lock,
  lfsUnlock: calls.unlock,
  lfsTrack: calls.track,
}));

import { LfsState } from '../src/state/lfs.svelte';

const mine = { id: '1', path: 'art/hero.psd', owner: 'Ada', ours: true };
const bobs = { id: '2', path: 'level.psd', owner: 'Bob', ours: false };

describe('LFS locks', () => {
  beforeEach(() => {
    for (const call of Object.values(calls)) call.mockReset();
  });

  it('knows each locked path and whose it is', async () => {
    calls.locks.mockResolvedValue([mine, bobs]);
    const lfs = new LfsState();
    await lfs.load('/repo');
    expect(lfs.byPath.get('level.psd')?.owner).toBe('Bob');
    expect(lfs.byPath.get('art/hero.psd')?.ours).toBe(true);
  });

  it('reads the locks again after locking, and passes on a refusal', async () => {
    calls.locks.mockResolvedValueOnce([]).mockResolvedValueOnce([mine]);
    calls.lock.mockRejectedValueOnce(new Error('already created lock'));
    const lfs = new LfsState();
    await lfs.load('/repo');
    expect(await lfs.lock('art/hero.psd')).toContain('already created lock');
    expect(lfs.locks).toEqual([mine]);
  });

  it('says why the locks could not be read, and shows none', async () => {
    calls.locks.mockRejectedValue(new Error('the LFS server did not answer within 15 seconds'));
    const lfs = new LfsState();
    await lfs.load('/repo');
    expect(lfs.locks).toBeNull();
    expect(lfs.error).toContain('did not answer');
  });
});
