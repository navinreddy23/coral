// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it } from 'vitest';

/**
 * The key a submodule is reached with.
 *
 * A submodule is a separate repository and git clones it in its own configuration, so it reads
 * none of the superproject's. The panel is where that is stated and where the choice is made.
 */
import Submodule from '../../src/app/Submodule.svelte';
import type { SshKey, Submodule as Sub, SubmoduleSsh } from '../../src/ipc/types';

afterEach(cleanup);

const SUB: Sub = {
  name: 'external/dev-scripts',
  path: 'external/dev-scripts',
  url: 'git@example.com:team/dev-scripts.git',
  pinned: '3e88757dd4883b678cf708eaf79c02fc358cf18b',
  initialised: true,
};

const KEYS: SshKey[] = [
  {
    path: '/home/someone/.ssh/id_ed25519',
    publicPath: '/home/someone/.ssh/id_ed25519.pub',
    comment: 'someone@workstation',
    kind: 'ssh-ed25519',
  },
  {
    path: '/home/someone/.ssh/id_work',
    publicPath: '/home/someone/.ssh/id_work.pub',
    comment: 'someone@company',
    kind: 'ssh-ed25519',
  },
];

function panel(ssh: SubmoduleSsh | null) {
  const chosen: (string | null)[] = [];
  const view = render(Submodule, {
    props: {
      submodule: SUB,
      revision: null,
      ssh,
      sshKeys: KEYS,
      busy: false,
      error: null,
      onClose: () => {},
      onSetUrl: () => {},
      onSetSshKey: (key: string | null) => chosen.push(key),
      onOpen: () => {},
      onUpdate: () => {},
      onRemove: () => {},
    },
  });
  const picker = view.container.querySelector('.sshkey select') as HTMLSelectElement;
  return { view, picker, chosen };
}

describe('which ssh key a submodule is reached with', () => {
  it('starts on what the repository uses, and names it', () => {
    const { picker } = panel({ key: null, inherited: '/home/someone/.ssh/id_work' });

    expect(picker.value).toBe('');
    expect(picker.options[0]?.textContent).toContain('id_work');
    expect(picker.options[0]?.textContent).toContain("This repository's key");
  });

  it('says the agent decides when the repository pins nothing', () => {
    const { picker } = panel({ key: null, inherited: '' });

    expect(picker.options[0]?.textContent).toContain('Whatever the agent offers');
  });

  it('shows the key this submodule was given, not the one it would inherit', () => {
    const { picker } = panel({
      key: '/home/someone/.ssh/id_ed25519',
      inherited: '/home/someone/.ssh/id_work',
    });

    expect(picker.value).toBe('/home/someone/.ssh/id_ed25519');
  });

  it('sends the chosen key, and null to put it back on the repository’s', async () => {
    const { picker, chosen } = panel({ key: null, inherited: '/home/someone/.ssh/id_work' });

    picker.value = '/home/someone/.ssh/id_ed25519';
    await fireEvent.change(picker);
    expect(chosen).toEqual(['/home/someone/.ssh/id_ed25519']);

    picker.value = '';
    await fireEvent.change(picker);
    expect(chosen).toEqual(['/home/someone/.ssh/id_ed25519', null]);
  });

  it('cannot be changed before the answer arrives', () => {
    // Null is "not read yet", and a picker sitting on the first option would be offering to
    // clear a pin nobody has seen.
    const { picker } = panel(null);

    expect(picker.disabled).toBe(true);
  });

  it('says the price of pinning, as both other screens that offer the choice do', () => {
    const { view } = panel({ key: null, inherited: '' });
    const note = view.container.querySelector('.note')?.textContent ?? '';

    expect(note).toContain('~/.ssh/config');
    expect(note).toContain('passphrase');
  });
});
