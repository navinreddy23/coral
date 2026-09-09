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

  // The same words as the toast beside it, from the module that owns them. Phrased twice, the
  // two disagreed: a rejected push read "stopped on conflicts" here and "was rejected" there.
  it('reports what it did', async () => {
    invoke.mockResolvedValue({ what: 'merge side', conflicted: false, message: '' });
    const actions = new ActionsState();
    await actions.run('/repo', { kind: 'merge', rev: 'side', mode: 'auto' });
    expect(actions.report).toEqual({ text: 'merge side complete', tone: 'ok', what: null });
    expect(actions.busy).toBe(false);
  });

  it('says so when the operation stopped on conflicts', async () => {
    invoke.mockResolvedValue({ what: 'merge side', conflicted: true, message: '' });
    const actions = new ActionsState();
    await actions.run('/repo', { kind: 'merge', rev: 'side', mode: 'auto' });
    expect(actions.report).toEqual({ text: 'merge side stopped on conflicts', tone: 'warn', what: null });
  });

  it('calls a rejected push rejected, not a conflict', async () => {
    // The line phrased this itself and the toast beside it phrased it another way, so a
    // rejected push read "push v1.0 stopped on conflicts" here and "was rejected" there.
    // Nothing conflicted with anything: the remote already had that name.
    invoke.mockResolvedValue({
      what: 'push v1.0',
      conflicted: true,
      message: 'refs/tags/v1.0 -> refs/tags/v1.0 [rejected] (already exists)',
    });
    const actions = new ActionsState();
    await actions.run('/repo', {
      kind: 'push',
      remote: 'origin',
      setUpstream: false,
      refspec: 'refs/tags/v1.0',
      tags: false,
      forceWithLease: false,
      delete: false,
    });
    expect(actions.report).toEqual({ text: 'push v1.0 was rejected', tone: 'warn', what: null });
  });

  it('does not call a pull that brought nothing a failure', async () => {
    invoke.mockResolvedValue({
      what: 'pull',
      conflicted: false,
      message: 'Already up to date.',
    });
    const actions = new ActionsState();
    await actions.run('/repo', { kind: 'pull', remote: null, mode: 'rebase' });
    expect(actions.report).toEqual({ text: 'pull: already up to date', tone: 'ok', what: null });
  });

  it('reports a failure rather than throwing at the caller', async () => {
    invoke.mockImplementation(async () => {
      throw new Error('not a valid ref');
    });
    const actions = new ActionsState();
    const outcome = await actions.run('/repo', { kind: 'checkout', rev: 'nope' });
    expect(outcome).toBeNull();
    expect(actions.report).toEqual({ text: 'not a valid ref', tone: 'error', what: null });
    // Still usable: a failed action must not wedge the toolbar.
    expect(actions.busy).toBe(false);
  });

  /**
   * The engine names every action for the journal, and a failure now carries that name. It is
   * what the window titles the red toast with, in place of "Something went wrong" — which
   * tells the reader only what the colour already told them.
   */
  it('keeps the name the engine gave the operation that failed', async () => {
    invoke.mockImplementation(async () => {
      // A Tauri command rejects with the serialized error, not with an `Error`.
      // eslint-disable-next-line no-throw-literal
      throw {
        code: 'git_error',
        message: "git tag exited with 128: fatal: 'a release' is not a valid tag name.",
        what: 'tag a release',
      };
    });
    const actions = new ActionsState();
    await actions.run('/repo', { kind: 'tagCreate', name: 'a release', at: null, message: null });
    expect(actions.report).toEqual({
      text: "'a release' is not a valid tag name.",
      tone: 'error',
      what: 'tag a release',
    });
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
