// @vitest-environment happy-dom
import { cleanup, render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { open } from '@tauri-apps/plugin-dialog';
import Ssh from '../../src/app/Ssh.svelte';
import Profiles from '../../src/app/Profiles.svelte';
import { SshState } from '../../src/state/ssh.svelte';
import { ProfilesState } from '../../src/state/profiles.svelte';
import type { SshKey } from '../../src/ipc/types';

const dialog = vi.mocked(open);
afterEach(cleanup);

const FAR: SshKey = {
  path: 'D:\\keys\\work',
  publicPath: 'D:\\keys\\work.pub',
  comment: 'dev@company',
  kind: 'ssh-ed25519',
  tooOpen: false,
};

const AGENT = {
  useAgent: true,
  privateKey: '',
  publicKey: '',
  command: '',
  credentialHelper: '',
};

const SCOPES = {
  effective: AGENT,
  global: AGENT,
  local: { privateKey: null, publicKey: null, credentialHelper: null },
};

const BLANK = {
  user: { name: 'Dev', email: 'dev@example.com' },
  ssh: { privateKey: null, publicKey: null, credentialHelper: 'store' },
  signing: { format: null, program: null, key: null, signCommits: null, signTags: null },
};

const calls: { cmd: string; args: unknown }[] = [];

function wire() {
  calls.length = 0;
  invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case 'ssh_read':
      case 'ssh_set_repo':
      case 'ssh_set_app':
        return SCOPES;
      case 'ssh_keys':
        return [];
      case 'ssh_directory':
        return 'C:\\Users\\Dev\\.ssh';
      case 'ssh_key_at':
        if (args?.path === FAR.path) return FAR;
        throw { code: 'refused', message: `${String(args?.path)} does not exist` };
      case 'profile_list':
      case 'profile_set_settings':
        return {
          current: 'work',
          profiles: [{ id: 'work', name: 'Work', colour: 'lane3', settings: BLANK }],
        };
      default:
        throw new Error(`unstubbed ${cmd}`);
    }
  });
}

beforeEach(() => {
  invoke.mockReset();
  dialog.mockReset();
  localStorage.clear();
  wire();
});

async function sshPane() {
  const ssh = new SshState();
  await ssh.load('/work/repo');
  const view = render(Ssh, { props: { ssh, onCopied: () => undefined } });
  return { view, ssh };
}

describe('the ssh settings pane', () => {
  it('browses for a private key and fills both halves', async () => {
    dialog.mockResolvedValue(FAR.path);
    const { view } = await sshPane();
    await fireEvent.click(view.getByLabelText('Use the local ssh agent'));
    await fireEvent.click(view.getByText('Browse…'));

    const [priv, pub] = [...view.container.querySelectorAll('.row input')] as HTMLInputElement[];
    await waitFor(() => {
      if (priv?.value !== FAR.path) throw new Error(`private is ${priv?.value}`);
    });
    expect(pub?.value).toBe(FAR.publicPath);
    expect(dialog.mock.calls[0]?.[0]).toMatchObject({ defaultPath: 'C:\\Users\\Dev\\.ssh' });

    await fireEvent.click(view.getByText('Save for this repository'));
    await waitFor(() => {
      if (!calls.some((c) => c.cmd === 'ssh_set_repo')) throw new Error('not saved');
    });
    expect(calls.find((c) => c.cmd === 'ssh_set_repo')?.args).toMatchObject({
      overrides: { privateKey: FAR.path, publicKey: FAR.publicPath },
    });
  });

  it('refuses to save a typed path that is not a key, and says why', async () => {
    const { view } = await sshPane();
    await fireEvent.click(view.getByLabelText('Use the local ssh agent'));
    const priv = view.container.querySelector('.row input') as HTMLInputElement;
    await fireEvent.input(priv, { target: { value: '/home/dev/.ssh/id_typo' } });
    await fireEvent.click(view.getByText('Save for this repository'));

    const alert = await view.findByRole('alert');
    expect(alert.textContent).toContain('/home/dev/.ssh/id_typo does not exist');
    expect(calls.some((c) => c.cmd === 'ssh_set_repo')).toBe(false);
  });

  it('saves the agent without checking any file', async () => {
    const { view } = await sshPane();
    await fireEvent.click(view.getByText('Every repository'));
    await fireEvent.click(view.getByText('Save for every repository'));
    await waitFor(() => {
      if (!calls.some((c) => c.cmd === 'ssh_set_app')) throw new Error('not saved');
    });
    expect(calls.some((c) => c.cmd === 'ssh_key_at')).toBe(false);
  });
});

describe('a profile’s ssh key', () => {
  it('is chosen from anywhere and saved with the profile', async () => {
    dialog.mockResolvedValue(FAR.path);
    const profiles = new ProfilesState();
    await profiles.load();
    const ssh = new SshState();
    const view = render(Profiles, {
      props: {
        profiles,
        ssh,
        repository: null,
        identity: null,
        onSwitch: () => undefined,
        onDelete: () => undefined,
        onApplyHere: () => undefined,
      },
    });

    const save = view.getByText('Save') as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await fireEvent.click(view.getByText('Choose a key file…'));
    await waitFor(() => {
      if (save.disabled) throw new Error('nothing changed yet');
    });
    await fireEvent.click(save);

    await waitFor(() => {
      if (!calls.some((c) => c.cmd === 'profile_set_settings')) throw new Error('not saved');
    });
    expect(calls.find((c) => c.cmd === 'profile_set_settings')?.args).toEqual({
      id: 'work',
      settings: {
        ...BLANK,
        // Both halves change together; the credential helper is about the host and stays.
        ssh: { privateKey: FAR.path, publicKey: FAR.publicPath, credentialHelper: 'store' },
      },
    });
  });
});
