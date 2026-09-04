import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const { ActionsState } = await import('../src/state/actions.svelte');

describe('running an action', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('reports what it did', async () => {
    invoke.mockResolvedValue({ what: 'merge side', conflicted: false });
    const actions = new ActionsState();
    await actions.run('/repo', { kind: 'merge', rev: 'side' });
    expect(actions.report).toEqual({ text: 'merge side', tone: 'ok' });
    expect(actions.busy).toBe(false);
  });

  it('says so when the operation stopped on conflicts', async () => {
    invoke.mockResolvedValue({ what: 'merge side', conflicted: true });
    const actions = new ActionsState();
    await actions.run('/repo', { kind: 'merge', rev: 'side' });
    expect(actions.report).toEqual({ text: 'merge side stopped on conflicts', tone: 'warn' });
  });

  it('reports a failure rather than throwing at the caller', async () => {
    invoke.mockImplementation(async () => {
      throw new Error('not a valid ref');
    });
    const actions = new ActionsState();
    const outcome = await actions.run('/repo', { kind: 'checkout', rev: 'nope' });
    expect(outcome).toBeNull();
    expect(actions.report).toEqual({ text: 'not a valid ref', tone: 'error' });
    // Still usable: a failed action must not wedge the toolbar.
    expect(actions.busy).toBe(false);
  });

  it('refuses a second action while one is running', async () => {
    let release = () => {};
    invoke.mockReturnValue(new Promise((r) => (release = () => r({ what: 'pull', conflicted: false }))));
    const actions = new ActionsState();
    const first = actions.run('/repo', { kind: 'pull', remote: null, mode: 'ffOnly' });
    // Two mutations at once contend for index.lock and fail with a message about a lock file.
    expect(await actions.run('/repo', { kind: 'fetch', remote: null })).toBeNull();
    expect(invoke).toHaveBeenCalledTimes(1);
    release();
    await first;
  });
});
