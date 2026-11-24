import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const { retryAfterTrust } = await import('../src/state/hosttrust');
const { ActionsState } = await import('../src/state/actions.svelte');
const { StartState } = await import('../src/state/start.svelte');
const { hostKeyOf } = await import('../src/ipc/error');

const key = { host: '[git.example.com]:2222', kind: 'ED25519', fingerprint: 'SHA256:abc+/9' };
const unknownHost = {
  code: 'unknown_host_key',
  message: 'ssh has not connected to [git.example.com]:2222 before',
  hostKey: key,
};

// How the window calls it: the first attempt on its own, the retry only after a failure.
function withHostTrust<T>(op: () => Promise<T>, ask: (k: typeof key) => Promise<boolean>) {
  return op().catch((e: unknown) => retryAfterTrust(e, op, ask));
}

describe('a host ssh has not connected to before', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('is read off the engine’s error, and nothing else is', () => {
    expect(hostKeyOf(unknownHost)).toEqual(key);
    expect(hostKeyOf({ code: 'git_error', message: 'Host key verification failed.' })).toBeNull();
    expect(hostKeyOf({ code: 'unknown_host_key', hostKey: { host: 'h' } })).toBeNull();
    expect(hostKeyOf(new Error('boom'))).toBeNull();
  });

  it('is trusted and the operation run again once the user says so', async () => {
    const op = vi.fn().mockRejectedValueOnce(unknownHost).mockResolvedValueOnce('done');
    const ask = vi.fn().mockResolvedValue(true);
    await expect(withHostTrust(op, ask)).resolves.toBe('done');
    expect(ask).toHaveBeenCalledWith(key);
    expect(invoke).toHaveBeenCalledWith('ssh_trust_host_key', {
      host: key.host,
      fingerprint: key.fingerprint,
    });
    expect(op).toHaveBeenCalledTimes(2);
  });

  it('is not trusted, and the failure stands, when the user says no', async () => {
    const op = vi.fn().mockRejectedValue(unknownHost);
    await expect(withHostTrust(op, () => Promise.resolve(false))).rejects.toBe(unknownHost);
    expect(invoke).not.toHaveBeenCalled();
    expect(op).toHaveBeenCalledTimes(1);
  });

  // An ssh that ignores SSH_ASKPASS_REQUIRE asks the same question however it is answered.
  it('is asked about once, so an ssh that will not listen cannot go round for ever', async () => {
    const op = vi.fn().mockRejectedValue(unknownHost);
    const ask = vi.fn().mockResolvedValue(true);
    await expect(withHostTrust(op, ask)).rejects.toBe(unknownHost);
    expect(ask).toHaveBeenCalledTimes(1);
    expect(op).toHaveBeenCalledTimes(2);
  });

  // A superproject's host trusted, and then a submodule on a host of its own.
  it('asks again about a second host met on the way', async () => {
    const other = { ...unknownHost, hostKey: { ...key, host: 'gitlab.com', fingerprint: 'SHA256:x' } };
    const op = vi
      .fn()
      .mockRejectedValueOnce(unknownHost)
      .mockRejectedValueOnce(other)
      .mockResolvedValueOnce('done');
    const ask = vi.fn().mockResolvedValue(true);
    await expect(withHostTrust(op, ask)).resolves.toBe('done');
    expect(ask).toHaveBeenCalledTimes(2);
  });

  it('leaves every other failure alone', async () => {
    const refused = { code: 'git_error', message: 'Permission denied (publickey).' };
    const ask = vi.fn();
    await expect(withHostTrust(() => Promise.reject(refused), ask)).rejects.toBe(refused);
    expect(ask).not.toHaveBeenCalled();
  });

  it('is asked about by a fetch the user started', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'ssh_trust_host_key') return Promise.resolve();
      const trusted = invoke.mock.calls.some(([c]) => c === 'ssh_trust_host_key');
      return trusted
        ? Promise.resolve({ what: 'fetch', conflicted: false, message: '' })
        : Promise.reject(unknownHost);
    });
    const ask = vi.fn().mockResolvedValue(true);
    const actions = new ActionsState(ask);
    await actions.run('/repo', { kind: 'fetch', remote: null });
    expect(ask).toHaveBeenCalledWith(key);
    expect(actions.report?.tone).toBe('ok');
  });

  // A fetch on a timer must never put a question in front of somebody doing something else.
  it('is not asked about by a fetch the window started on its own', async () => {
    invoke.mockRejectedValue(unknownHost);
    const ask = vi.fn().mockResolvedValue(true);
    const actions = new ActionsState(ask);
    await expect(
      actions.quietly('/repo', { kind: 'fetch', remote: null }),
    ).rejects.toBe(unknownHost);
    expect(ask).not.toHaveBeenCalled();
  });

  it('is asked about by a clone', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'ssh_trust_host_key') return Promise.resolve();
      const trusted = invoke.mock.calls.some(([c]) => c === 'ssh_trust_host_key');
      return trusted ? Promise.resolve({ at: '/into/project', notes: [] }) : Promise.reject(unknownHost);
    });
    const ask = vi.fn().mockResolvedValue(true);
    const start = new StartState(ask);
    const made = await start.clone({
      url: 'ssh://git@git.example.com:2222/team/project.git',
      parent: '/into',
      name: '',
      sshKey: '/keys/bob',
      history: 'full',
      depth: 1,
      sparse: false,
    });
    expect(made).toBe('/into/project');
    expect(start.error).toBeNull();
  });
});
