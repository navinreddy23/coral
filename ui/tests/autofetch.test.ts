// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import { AutoFetchState } from '../src/state/autofetch.svelte';
import { ActionsState } from '../src/state/actions.svelte';

const MINUTE = 60_000;

beforeEach(() => {
  vi.useFakeTimers();
  localStorage.clear();
  invoke.mockReset();
  vi.spyOn(document, 'hasFocus').mockReturnValue(true);
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('fetching on a timer', () => {
  it('waits a whole interval after opening, then fetches once per interval', async () => {
    const auto = new AutoFetchState();
    const run = vi.fn(async () => undefined);
    auto.follow('/repo', run);
    expect(auto.every).toBe(10);

    await vi.advanceTimersByTimeAsync(9 * MINUTE);
    expect(run).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(MINUTE + 30_000);
    expect(run).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledWith('/repo');
    await vi.advanceTimersByTimeAsync(5 * MINUTE);
    expect(run).toHaveBeenCalledTimes(1);
    auto.stop();
  });

  it('counts from the last fetch, whoever asked for it', async () => {
    const auto = new AutoFetchState();
    const run = vi.fn(async () => undefined);
    auto.follow('/repo', run);
    await vi.advanceTimersByTimeAsync(8 * MINUTE);
    auto.fetched('/repo');
    await vi.advanceTimersByTimeAsync(8 * MINUTE);
    expect(run).not.toHaveBeenCalled();
    auto.stop();
  });

  it('does nothing while the window is in the background, and catches up on focus', async () => {
    const auto = new AutoFetchState();
    const run = vi.fn(async () => undefined);
    const focus = vi.spyOn(document, 'hasFocus').mockReturnValue(false);
    auto.follow('/repo', run);
    await vi.advanceTimersByTimeAsync(30 * MINUTE);
    expect(run).not.toHaveBeenCalled();

    focus.mockReturnValue(true);
    window.dispatchEvent(new Event('focus'));
    await vi.advanceTimersByTimeAsync(0);
    expect(run).toHaveBeenCalledTimes(1);
    auto.stop();
  });

  it('follows the repository’s own setting over the window’s, and never fetches when off', async () => {
    const auto = new AutoFetchState();
    const run = vi.fn(async () => undefined);
    auto.follow('/repo', run);
    auto.override = { kind: 'every', minutes: 1 };
    expect(auto.every).toBe(1);
    await vi.advanceTimersByTimeAsync(90_000);
    expect(run).toHaveBeenCalledTimes(1);

    auto.override = { kind: 'off' };
    await vi.advanceTimersByTimeAsync(60 * MINUTE);
    expect(run).toHaveBeenCalledTimes(1);
    auto.stop();
  });

  it('remembers the window’s interval and refuses one it does not offer', () => {
    new AutoFetchState().setMinutes(30);
    expect(new AutoFetchState().minutes).toBe(30);
    localStorage.setItem('coral.autoFetch', '7');
    expect(new AutoFetchState().minutes).toBe(10);
  });

  it('says a failure once, until a fetch gets through again', () => {
    const auto = new AutoFetchState();
    expect(auto.settled('could not resolve host')).toBe(true);
    expect(auto.settled('could not resolve host')).toBe(false);
    expect(auto.settled(null)).toBe(false);
    expect(auto.settled('could not resolve host')).toBe(true);
  });
});

describe('a background action', () => {
  it('holds nothing the user is waiting on, and the user’s action waits for it', async () => {
    let finish: (v: unknown) => void = () => undefined;
    const order: string[] = [];
    invoke.mockImplementation(async (_cmd: string, args: { action: { kind: string } }) => {
      order.push(`start ${args.action.kind}`);
      if (args.action.kind === 'fetch') await new Promise((r) => (finish = r));
      order.push(`end ${args.action.kind}`);
      return { what: args.action.kind, conflicted: false, message: '' };
    });
    const actions = new ActionsState();
    const background = actions.quietly('/repo', { kind: 'fetch', remote: null, background: true });
    expect(actions.busy).toBe(false);
    expect(actions.quiet).toBe(true);

    const mine = actions.run('/repo', { kind: 'checkout', rev: 'main' });
    await vi.advanceTimersByTimeAsync(0);
    expect(order).toEqual(['start fetch']);

    finish(undefined);
    await background;
    await mine;
    expect(order).toEqual(['start fetch', 'end fetch', 'start checkout', 'end checkout']);
    expect(actions.quiet).toBe(false);
    expect(actions.report?.text).toContain('checkout');
  });

  it('does not run while the user’s action does', async () => {
    invoke.mockImplementation(() => new Promise(() => undefined));
    const actions = new ActionsState();
    void actions.run('/repo', { kind: 'checkout', rev: 'main' });
    expect(await actions.quietly('/repo', { kind: 'fetch', remote: null, background: true })).toBeNull();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0]?.[1]).toMatchObject({ action: { kind: 'checkout' } });
  });
});
