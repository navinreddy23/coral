// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
const invoke = vi.hoisted(() => vi.fn());
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { open } from '@tauri-apps/plugin-dialog';
import { SshState } from '../src/state/ssh.svelte';
import type { SshKey } from '../src/ipc/types';

const dialog = vi.mocked(open);

const HOME_KEY: SshKey = {
  path: '/home/dev/.ssh/id_ed25519',
  publicPath: '/home/dev/.ssh/id_ed25519.pub',
  comment: 'dev@home',
  kind: 'ssh-ed25519',
  tooOpen: false,
};

const FAR_KEY: SshKey = {
  path: 'D:\\keys\\work',
  publicPath: null,
  comment: '',
  kind: 'private key',
  tooOpen: false,
};

/** Answers the ssh commands; `onDisk` is which chosen paths still describe a key. */
function wire(onDisk: Record<string, SshKey | Error>) {
  invoke.mockImplementation(async (cmd: string, args?: { path?: string }) => {
    if (cmd === 'ssh_keys') return [HOME_KEY];
    if (cmd === 'ssh_directory') return '/home/dev/.ssh';
    if (cmd === 'ssh_key_at') {
      const found = onDisk[args?.path ?? ''];
      if (found === undefined) throw { code: 'refused', message: `${args?.path} does not exist` };
      if (found instanceof Error) throw { code: 'refused', message: found.message };
      return found;
    }
    throw new Error(`unstubbed ${cmd}`);
  });
}

beforeEach(() => {
  invoke.mockReset();
  dialog.mockReset();
  localStorage.clear();
});

describe('a key chosen from anywhere', () => {
  it('opens the picker in the ssh folder, with no extension filter', async () => {
    wire({ 'D:\\keys\\work': FAR_KEY });
    dialog.mockResolvedValue('D:\\keys\\work');
    const ssh = new SshState();
    await ssh.loadKeys();

    const got = await ssh.chooseFile();
    expect(got).toEqual({ key: FAR_KEY });
    const asked = dialog.mock.calls[0]?.[0];
    expect(asked).toMatchObject({ defaultPath: '/home/dev/.ssh', directory: false });
    // Private keys usually have no extension, so a filter would hide exactly what is wanted.
    expect(asked).not.toHaveProperty('filters');
    expect(ssh.keys.map((k) => k.path)).toEqual([HOME_KEY.path, FAR_KEY.path]);
  });

  it('is offered again after a restart, and forgotten once it has gone', async () => {
    wire({ 'D:\\keys\\work': FAR_KEY });
    dialog.mockResolvedValue('D:\\keys\\work');
    await new SshState().chooseFile();

    const later = new SshState();
    await later.loadKeys();
    expect(later.keys.map((k) => k.path)).toContain(FAR_KEY.path);

    // The stick it lived on was unplugged.
    wire({});
    const after = new SshState();
    await after.loadKeys();
    expect(after.keys.map((k) => k.path)).toEqual([HOME_KEY.path]);
    expect(localStorage.getItem('coral.ssh.chosen')).toBe('[]');
  });

  it('says why a file is not a key, and lists nothing', async () => {
    wire({ 'C:\\keys\\work.ppk': new Error('C:\\keys\\work.ppk is a PuTTY key. Open it in PuTTYgen') });
    dialog.mockResolvedValue('C:\\keys\\work.ppk');
    const ssh = new SshState();
    await ssh.loadKeys();
    const got = await ssh.chooseFile();
    expect(got).toMatchObject({ error: expect.stringContaining('PuTTYgen') });
    expect(ssh.keys).toEqual([HOME_KEY]);
  });

  it('changes nothing when the picker is closed', async () => {
    wire({});
    dialog.mockResolvedValue(null);
    const ssh = new SshState();
    await ssh.loadKeys();
    expect(await ssh.chooseFile()).toBeNull();
    expect(invoke).not.toHaveBeenCalledWith('ssh_key_at', expect.anything());
  });

  it('survives storage that holds nonsense', async () => {
    localStorage.setItem('coral.ssh.chosen', '{not json');
    wire({});
    const ssh = new SshState();
    await ssh.loadKeys();
    expect(ssh.keys).toEqual([HOME_KEY]);
  });

  it('does not list a key in the ssh folder twice', async () => {
    wire({ [HOME_KEY.path]: HOME_KEY });
    const ssh = new SshState();
    await ssh.loadKeys();
    await ssh.adopt(HOME_KEY.path);
    expect(ssh.keys).toHaveLength(1);
  });
});
