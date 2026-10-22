import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { ReflogState } from '../src/state/reflog.svelte';
import type { ReflogEntry } from '../src/ipc/types';

function entry(index: number, oid: string, lost: boolean): ReflogEntry {
  return { oid, index, action: 'commit: x', time: 1_761_000_000 - index, summary: 'x', lost };
}

beforeEach(() => {
  invoke.mockReset();
});

describe('where HEAD has been', () => {
  it('counts each lost commit once, however often HEAD was moved to it', async () => {
    invoke.mockResolvedValue([entry(0, 'a', true), entry(1, 'b', false), entry(2, 'a', true)]);
    const log = new ReflogState();
    await log.load('/repo');
    expect(invoke).toHaveBeenCalledWith('repo_reflog', { path: '/repo' });
    expect(log.lostCount).toBe(1);
    expect(log.shown).toHaveLength(3);
    log.onlyLost = true;
    expect(log.shown.map((e) => e.index)).toEqual([0, 2]);
  });

  it('drops an answer for a repository that is no longer the one wanted', async () => {
    let answer: (v: ReflogEntry[]) => void = () => undefined;
    invoke.mockReturnValueOnce(new Promise((r) => (answer = r)));
    invoke.mockResolvedValueOnce([entry(0, 'b', false)]);
    const log = new ReflogState();
    const first = log.load('/old');
    await log.load('/new');
    answer([entry(0, 'a', true)]);
    await first;
    expect(log.entries.map((e) => e.oid)).toEqual(['b']);
    expect(log.loading).toBe(false);
  });

  it('says why when it cannot be read', async () => {
    invoke.mockRejectedValue({ code: 'git', message: 'fatal: bad' });
    const log = new ReflogState();
    await log.load('/repo');
    expect(log.entries).toEqual([]);
    expect(log.error).toContain('bad');
  });
});
