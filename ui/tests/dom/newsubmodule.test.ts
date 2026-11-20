// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it } from 'vitest';

/**
 * The form that adds a submodule: a URL, the path git would derive from it until one is typed,
 * an optional branch and key, and a word of warning before git refuses a local path.
 */
import NewSubmodule, { type SubmoduleRequest } from '../../src/app/NewSubmodule.svelte';
import type { SshKey } from '../../src/ipc/types';

afterEach(cleanup);

const KEYS: SshKey[] = [
  {
    path: '/home/someone/.ssh/id_work',
    publicPath: '/home/someone/.ssh/id_work.pub',
    comment: 'someone@company',
    kind: 'ssh-ed25519',
    tooOpen: false,
  },
];

function sheet(sshKeys: SshKey[] = KEYS, busy = false) {
  const added: SubmoduleRequest[] = [];
  const view = render(NewSubmodule, {
    props: {
      sshKeys,
      busy,
      error: null,
      onClose: () => {},
      onAdd: (request: SubmoduleRequest) => added.push(request),
    },
  });
  const inputs = [...view.container.querySelectorAll('input')] as HTMLInputElement[];
  const [url, path, branch] = inputs as [HTMLInputElement, HTMLInputElement, HTMLInputElement];
  const button = view.container.querySelector('button.primary') as HTMLButtonElement;
  return { view, added, url, path, branch, button };
}

describe('adding a submodule', () => {
  it('cannot be asked for without a URL', () => {
    const { button } = sheet();
    expect(button.disabled).toBe(true);
  });

  it('fills the path in from the URL, as git would derive it', async () => {
    const { url, path, button, added } = sheet();
    await fireEvent.input(url, { target: { value: 'git@github.com:owner/dev-scripts.git' } });
    expect(path.value).toBe('dev-scripts');

    await fireEvent.click(button);
    expect(added).toEqual([
      { url: 'git@github.com:owner/dev-scripts.git', path: 'dev-scripts', branch: null, key: null },
    ]);
  });

  it('keeps a path once one has been typed, whatever the URL becomes', async () => {
    const { url, path, branch, button, added } = sheet();
    await fireEvent.input(url, { target: { value: 'https://e.x/lib.git' } });
    await fireEvent.input(path, { target: { value: 'vendor/lib' } });
    await fireEvent.input(url, { target: { value: 'https://e.x/other.git' } });
    expect(path.value).toBe('vendor/lib');

    await fireEvent.input(branch, { target: { value: ' dev ' } });
    const picker = document.querySelector('select') as HTMLSelectElement;
    await fireEvent.change(picker, { target: { value: '/home/someone/.ssh/id_work' } });
    await fireEvent.keyDown(path, { key: 'Enter' });
    expect(added).toEqual([
      {
        url: 'https://e.x/other.git',
        path: 'vendor/lib',
        branch: 'dev',
        key: '/home/someone/.ssh/id_work',
      },
    ]);
    expect(button.disabled).toBe(false);
  });

  it('says before the click that git refuses a path on this machine', async () => {
    const { url, view } = sheet();
    expect(view.container.textContent).not.toContain('protocol.file.allow');
    await fireEvent.input(url, { target: { value: '../sibling.git' } });
    expect(view.container.textContent).not.toContain('protocol.file.allow');
    await fireEvent.input(url, { target: { value: '/srv/git/lib.git' } });
    expect(view.container.textContent).toContain('protocol.file.allow');
  });

  it('offers no key picker on a machine with no keys', () => {
    const { view } = sheet([]);
    expect(view.container.querySelector('select')).toBeNull();
  });

  it('cannot be sent twice while the first is running', () => {
    const { button } = sheet(KEYS, true);
    expect(button.disabled).toBe(true);
    expect(button.textContent).toContain('Adding');
  });
});
