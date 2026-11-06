// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it } from 'vitest';

import SshKeyField from '../../src/app/SshKeyField.svelte';
import type { SshKey } from '../../src/ipc/types';

afterEach(cleanup);

const COPIED: SshKey = {
  path: '/media/stick/id_copied',
  publicPath: null,
  comment: '',
  kind: 'private key',
  tooOpen: true,
};

const WINDOWS: SshKey = {
  path: 'C:\\Users\\Dev\\Documents\\keys\\work',
  publicPath: 'C:\\Users\\Dev\\Documents\\keys\\work.pub',
  comment: 'dev@company',
  kind: 'ssh-ed25519',
  tooOpen: false,
};

function field(keys: SshKey[], value: string) {
  const changed: string[] = [];
  const view = render(SshKeyField, {
    props: { keys, value, onChange: (p: string) => changed.push(p), onChooseFile: async () => null },
  });
  return { view, changed };
}

describe('the ssh key field', () => {
  it('warns that ssh will refuse a key other users can read', () => {
    const { view } = field([COPIED], COPIED.path);
    const alert = view.getByRole('alert');
    expect(alert.textContent).toContain('ssh will refuse it');
    expect(alert.textContent).toContain('chmod 600');
    // The path is said once, beneath, rather than again inside the warning.
    expect(view.container.querySelector('.where')?.textContent).toBe('/media/stick/id_copied');
  });

  it('names a Windows key by its file, and says the whole path beneath', () => {
    const { view } = field([WINDOWS], WINDOWS.path);
    const option = [...view.container.querySelectorAll('option')].find(
      (o) => o.value === WINDOWS.path,
    );
    expect(option?.textContent).toBe('work · dev@company');
    expect(view.container.querySelector('.where')?.textContent).toBe(WINDOWS.path);
    expect(view.queryByRole('alert')).toBeNull();
  });

  it('keeps a key set elsewhere selectable even when this machine has no such file', () => {
    // A profile synced from another machine, or a path typed into the settings by hand.
    const { view } = field([WINDOWS], '/home/dev/.ssh/id_gone');
    const select = view.container.querySelector('select') as HTMLSelectElement;
    expect(select.value).toBe('/home/dev/.ssh/id_gone');
  });

  it('goes back to the agent', async () => {
    const { view, changed } = field([WINDOWS], WINDOWS.path);
    const select = view.container.querySelector('select') as HTMLSelectElement;
    select.value = '';
    await fireEvent.change(select);
    expect(changed).toEqual(['']);
  });
});
