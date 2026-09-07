// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));

const { TransferState } = await import('../src/state/transfer.svelte');

function report(over: Record<string, unknown> = {}) {
  return {
    key: '/srv/thing',
    label: 'Fetch',
    state: 'running' as const,
    phase: 'Receiving objects',
    current: 40,
    total: 200,
    percent: 20,
    remote: false,
    detail: '',
    ...over,
  };
}

beforeEach(() => invoke.mockReset());

describe('what the window shows while something is talking to a server', () => {
  it('shows nothing until something starts', () => {
    const transfer = new TransferState();
    expect(transfer.current).toBeNull();
    expect(transfer.measured).toBe(false);
  });

  it('says the work has started before git has counted anything', () => {
    // The state a host that never answers stays in. A bar that claimed nought per cent would
    // be asserting progress it has no basis for, and the way out has to be offered here.
    const transfer = new TransferState();
    transfer.take(report({ phase: '', current: 0, total: 0, percent: 0, label: 'Clone' }));

    expect(transfer.current).not.toBeNull();
    expect(transfer.measured, 'nothing countable yet').toBe(false);
    expect(transfer.caption).toBe('Clone…');
  });

  it('says where the work is happening once git counts', () => {
    const transfer = new TransferState();
    transfer.take(report({ remote: true, phase: 'Compressing objects' }));

    expect(transfer.measured).toBe(true);
    expect(transfer.caption).toBe('Fetch: Compressing objects on the server');
  });

  it('clears itself when the transfer ends, however it ended', () => {
    for (const state of ['finished', 'failed', 'cancelled'] as const) {
      const transfer = new TransferState();
      transfer.take(report());
      transfer.take(report({ state }));
      expect(transfer.current, state).toBeNull();
    }
  });

  it('is not cleared by a report from a transfer it is not showing', () => {
    // Two repositories can be fetched one after another, and a late terminal report from the
    // first would otherwise take the bar away from the second.
    const transfer = new TransferState();
    transfer.take(report({ key: '/srv/second' }));
    transfer.take(report({ key: '/srv/first', state: 'finished' }));

    expect(transfer.current?.key).toBe('/srv/second');
  });

  it('cancels the one it is showing, and says so while it waits', async () => {
    invoke.mockResolvedValue(true);
    const transfer = new TransferState();
    transfer.take(report());

    const asked = transfer.cancel();
    expect(transfer.stopping, 'the button cannot be pressed twice').toBe(true);
    await asked;

    expect(invoke).toHaveBeenCalledWith('cancel_transfer', { key: '/srv/thing' });
    // Still showing: the bar goes when the engine says it stopped, not when we asked.
    expect(transfer.current).not.toBeNull();

    transfer.take(report({ state: 'cancelled' }));
    expect(transfer.current).toBeNull();
    expect(transfer.stopping).toBe(false);
  });

  it('cancels nothing when nothing is running', async () => {
    const transfer = new TransferState();
    await transfer.cancel();
    expect(invoke).not.toHaveBeenCalled();
  });
});
