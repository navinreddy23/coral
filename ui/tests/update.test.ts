// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const ipc = vi.hoisted(() => ({
  checkForUpdate: vi.fn(),
  installUpdate: vi.fn(),
  onUpdateProgress: vi.fn(),
}));
vi.mock('../src/ipc/update', () => ipc);

import { UpdateState } from '../src/state/update.svelte';

const NEWER = {
  current: '1.2.3',
  latest: '1.2.4',
  newer: true,
  url: 'https://github.com/navinreddy23/coral/releases/tag/v1.2.4',
  installable: true,
};

describe('checking for and installing an update', () => {
  beforeEach(() => {
    localStorage.clear();
    for (const f of Object.values(ipc)) f.mockReset();
    ipc.onUpdateProgress.mockResolvedValue(() => undefined);
  });

  it('does not check at launch unless it has been asked to', async () => {
    const updates = new UpdateState();
    expect(updates.automatic).toBe(false);
    expect(await updates.atLaunch()).toBeNull();
    expect(ipc.checkForUpdate).not.toHaveBeenCalled();
  });

  it('checks at launch once it has been asked to, and remembers being asked', async () => {
    new UpdateState().setAutomatic(true);
    ipc.checkForUpdate.mockResolvedValue(NEWER);
    const next = new UpdateState();
    expect(next.automatic).toBe(true);
    expect(await next.atLaunch()).toEqual(NEWER);
    expect(next.available).toBe(true);
  });

  it('keeps a failed check at launch to itself', async () => {
    // Nobody asked, so an unreachable GitHub is not worth an error in front of them.
    new UpdateState().setAutomatic(true);
    ipc.checkForUpdate.mockRejectedValue(new Error('offline'));
    const updates = new UpdateState();
    expect(await updates.atLaunch()).toBeNull();
    expect(updates.error).toBeNull();
    expect(updates.available).toBe(false);
  });

  it('reports a check that was asked for and failed', async () => {
    ipc.checkForUpdate.mockRejectedValue(new Error('could not reach the host'));
    const updates = new UpdateState();
    await updates.check();
    expect(updates.error).toContain('could not reach');
    expect(updates.checking).toBe(false);
  });

  it('says nothing is available when this is the newest release', async () => {
    ipc.checkForUpdate.mockResolvedValue({ ...NEWER, latest: '1.2.3', newer: false, installable: false });
    const updates = new UpdateState();
    await updates.check();
    expect(updates.available).toBe(false);
  });

  it('follows the download while it installs, and reports a failed install', async () => {
    const listener: { report?: (p: { received: number; total: number | null }) => void } = {};
    ipc.onUpdateProgress.mockImplementation(async (handle) => {
      listener.report = handle;
      return () => undefined;
    });
    let fail: (e: unknown) => void = () => undefined;
    ipc.installUpdate.mockReturnValue(new Promise((_, reject) => (fail = reject)));

    const updates = new UpdateState();
    const installing = updates.install();
    await vi.waitFor(() => expect(listener.report).toBeDefined());
    listener.report?.({ received: 512, total: 2048 });
    expect(updates.installing).toBe(true);
    expect(updates.fraction).toBeCloseTo(0.25);

    fail(new Error('the signature did not verify'));
    await installing;
    expect(updates.installing).toBe(false);
    expect(updates.error).toContain('signature');
  });
});
