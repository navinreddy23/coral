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

  it('keeps the one underneath, so nothing runs without a way to stop it', () => {
    // A clone is started from the start page and a fetch from the toolbar, and those two can
    // overlap. Showing only the newest left the other running invisibly, which means running
    // with no way to stop it — the one thing the bar exists to give.
    const transfer = new TransferState();
    transfer.take(report({ key: '/srv/clone', label: 'Clone' }));
    transfer.take(report({ key: '/srv/fetch', label: 'Fetch' }));

    expect(transfer.current?.key).toBe('/srv/fetch');
    expect(transfer.waiting).toBe(1);

    transfer.take(report({ key: '/srv/fetch', state: 'finished' }));
    expect(transfer.current?.key, 'the clone comes back into view').toBe('/srv/clone');
    expect(transfer.waiting).toBe(0);
  });

  it('updates in place rather than stacking every report', () => {
    const transfer = new TransferState();
    transfer.take(report({ percent: 10 }));
    transfer.take(report({ percent: 60 }));

    expect(transfer.running.length).toBe(1);
    expect(transfer.current?.percent).toBe(60);
  });

  it('offers the stop again when the ask itself fails', async () => {
    // A window left with a disabled button and a transfer still running is worse than one
    // that simply did not manage to stop it.
    //
    // `mockImplementationOnce`, because an implementation that outlives the test leaves vitest
    // holding a rejected promise nobody claimed and it fails the test for a reason that is not
    // the code's. One call is all this needs anyway.
    invoke.mockImplementationOnce(async () => {
      throw new Error('the window has gone');
    });
    const transfer = new TransferState();
    transfer.take(report());

    await transfer.cancel();

    expect(transfer.asked, 'the button is pressable again').toBe(false);
    expect(transfer.current, 'and the transfer is still there').not.toBeNull();
    expect(invoke, 'and it did ask').toHaveBeenCalledWith('cancel_transfer', {
      key: '/srv/thing',
    });
  });

  it('cancels the one it is showing, and says so while it waits', async () => {
    invoke.mockResolvedValue(true);
    const transfer = new TransferState();
    transfer.take(report());

    const asked = transfer.cancel();
    expect(transfer.asked, 'the button cannot be pressed twice').toBe(true);
    await asked;

    expect(invoke).toHaveBeenCalledWith('cancel_transfer', { key: '/srv/thing' });
    // Still showing: the bar goes when the engine says it stopped, not when we asked.
    expect(transfer.current).not.toBeNull();

    transfer.take(report({ state: 'cancelled' }));
    expect(transfer.current).toBeNull();
    expect(transfer.asked).toBe(false);
  });

  it('cancels nothing when nothing is running', async () => {
    const transfer = new TransferState();
    await transfer.cancel();
    expect(invoke).not.toHaveBeenCalled();
  });
});
