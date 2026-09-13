// @vitest-environment happy-dom
import { cleanup, render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import Start from '../../src/app/Start.svelte';
import { StartState } from '../../src/state/start.svelte';

afterEach(cleanup);

const KEYS = [
  {
    path: '/home/someone/.ssh/id_ed25519',
    publicPath: '/home/someone/.ssh/id_ed25519.pub',
    comment: 'someone@home',
    kind: 'ssh-ed25519',
  },
  {
    path: '/home/someone/.ssh/id_work',
    publicPath: '/home/someone/.ssh/id_work.pub',
    comment: 'someone@work',
    kind: 'ssh-ed25519',
  },
];

const RECENTS = [
  { path: '/home/someone/Work/alpha', name: 'alpha', opened: 1_756_000_000, missing: false },
  { path: '/home/someone/Play/beta', name: 'beta', opened: 1_755_000_000, missing: false },
];

function wire(over: Record<string, unknown> = {}) {
  const table: Record<string, unknown> = {
    recent_repos: RECENTS,
    forget_recent: [RECENTS[1]],
    lfs_available: true,
    repo_init: '/home/someone/Work/made',
    repo_clone: { at: '/home/someone/Work/cloned', notes: [] },
    ...over,
  };
  invoke.mockImplementation(async (cmd: string) => {
    if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
    const answer = table[cmd];
    if (answer instanceof Error) throw answer;
    return answer;
  });
}

/** Renders the page with its state already loaded, as the window shows it. */
async function page(over: Record<string, unknown> = {}, directory = '/home/someone/Work') {
  wire(over);
  const start = new StartState();
  const opened: string[] = [];
  const view = render(Start, {
    props: {
      start,
      onOpen: (path: string) => opened.push(path),
      onPickDirectory: async () => directory,
      onClose: null,
      sshKeys: KEYS,
      defaultSshKey: '',
      onConfirm: async () => true,
    },
  });
  await start.load();
  return { view, start, opened };
}

describe('the start page', () => {
  // Braces, not a bare expression: `mockReset` answers with the mock, vitest takes a returned
  // function as the test's teardown, and it then called the mock with no arguments at all.
  beforeEach(() => {
    invoke.mockReset();
  });

  it('lists what has been opened before, newest first', async () => {
    const { view } = await page();
    const names = [...view.container.querySelectorAll('.repo-name')].map((e) => e.textContent);
    expect(names).toEqual(['alpha', 'beta']);
  });

  it('filters on the name and on the path', async () => {
    const { view, start } = await page();

    start.filter = 'beta';
    await waitFor(() => {
      if (view.container.querySelectorAll('.repo-name').length !== 1) throw new Error('not yet');
    });
    expect(view.container.textContent).toContain('beta');

    // The path too: people remember where a repository is as often as what it is called.
    start.filter = 'Work';
    await waitFor(() => {
      const names = [...view.container.querySelectorAll('.repo-name')].map((e) => e.textContent);
      if (names.join() !== 'alpha') throw new Error('not yet');
    });
  });

  it('says so when the filter matches nothing, rather than showing an empty list', async () => {
    const { view, start } = await page();
    start.filter = 'nothing like this';
    await waitFor(() => {
      if (!view.container.textContent?.includes('Nothing matches')) throw new Error('not yet');
    });
  });

  it('marks a repository that is no longer where it was', async () => {
    // Without the mark the row reads like every other one and clicking it ends in "not a git
    // repository", which names a fault rather than the repository someone moved.
    const gone = [RECENTS[0], { ...RECENTS[1], missing: true }];
    const { view } = await page({ recent_repos: gone });

    await waitFor(() => {
      if (view.container.querySelectorAll('.recents li').length !== 2) throw new Error('not yet');
    });
    const [here, moved] = [...view.container.querySelectorAll('.recents li')];
    expect(here?.className).not.toContain('gone');
    expect(moved?.className).toContain('gone');
    expect(moved?.textContent).toContain('Not found');
    expect(moved?.querySelector('.repo')?.getAttribute('title')).toContain(
      'Nothing is at /home/someone/Play/beta any more',
    );
  });

  it('opens a recent repository by its row', async () => {
    const { view, opened } = await page();
    await fireEvent.click(view.getByText('alpha'));
    expect(opened).toEqual(['/home/someone/Work/alpha']);
  });

  it('creates one where it says it will, and opens what it made', async () => {
    const { view, opened } = await page();
    await fireEvent.click(view.getByText('Create'));

    await fireEvent.click(view.getByText('Choose…'));
    const name = view.getByPlaceholderText("the repository's name");
    await fireEvent.input(name, { target: { value: 'made' } });

    // Where it will be, before it is made: a create button that lands somewhere unexpected is
    // a directory nobody asked for.
    await waitFor(() => {
      if (!view.container.textContent?.includes('/home/someone/Work/made')) {
        throw new Error('not yet');
      }
    });

    await fireEvent.click(view.getByText('Create', { selector: '.primary' }));
    await waitFor(() => {
      if (opened.length === 0) throw new Error('not yet');
    });

    expect(invoke).toHaveBeenCalledWith('repo_init', {
      path: '/home/someone/Work/made',
      branch: 'main',
      lfs: false,
    });
    expect(opened).toEqual(['/home/someone/Work/made']);
  });

  it('leaves git to choose the first branch when the field is emptied', async () => {
    // `--initial-branch=` is an error, and the user may have configured a default that Coral
    // has no business overriding.
    const { view } = await page();
    await fireEvent.click(view.getByText('Create'));
    await fireEvent.click(view.getByText('Choose…'));
    await fireEvent.input(view.getByPlaceholderText("the repository's name"), {
      target: { value: 'made' },
    });
    await fireEvent.input(view.getByPlaceholderText("leave empty for git's default"), {
      target: { value: '   ' },
    });
    await fireEvent.click(view.getByText('Create', { selector: '.primary' }));

    await waitFor(() => {
      if (!invoke.mock.calls.some(([cmd]) => cmd === 'repo_init')) throw new Error('not yet');
    });
    expect(invoke).toHaveBeenCalledWith('repo_init', {
      path: '/home/someone/Work/made',
      branch: null,
      lfs: false,
    });
  });

  it('offers Large File Storage only where it is installed', async () => {
    const withIt = await page();
    await fireEvent.click(withIt.view.getByText('Create'));
    expect(withIt.view.container.textContent).toContain('Large File Storage');

    cleanup();

    const without = await page({ lfs_available: false });
    await fireEvent.click(without.view.getByText('Create'));
    expect(without.view.container.textContent).not.toContain('Large File Storage');
  });

  it('asks for Large File Storage when it is ticked', async () => {
    const { view } = await page();
    await fireEvent.click(view.getByText('Create'));
    await fireEvent.click(view.getByText('Choose…'));
    await fireEvent.input(view.getByPlaceholderText("the repository's name"), {
      target: { value: 'made' },
    });
    await fireEvent.click(view.getByLabelText('Set up Large File Storage in it'));
    await fireEvent.click(view.getByText('Create', { selector: '.primary' }));

    await waitFor(() => {
      if (!invoke.mock.calls.some(([cmd]) => cmd === 'repo_init')) throw new Error('not yet');
    });
    expect(invoke).toHaveBeenCalledWith('repo_init', {
      path: '/home/someone/Work/made',
      branch: 'main',
      lfs: true,
    });
  });

  it('will not clone without somewhere to put it', async () => {
    const { view } = await page();
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'https://host/team/thing.git' },
    });

    const button = view.getByText('Clone', { selector: '.primary' });
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });

  it('names the directory the clone will land in before it fetches anything', async () => {
    const { view, opened } = await page();
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'git@gitlab.com:open-source-23/coral.git' },
    });
    await fireEvent.click(view.getByText('Choose…'));

    await waitFor(() => {
      if (!view.container.textContent?.includes('/home/someone/Work/coral')) {
        throw new Error('not yet');
      }
    });

    await fireEvent.click(view.getByText('Clone', { selector: '.primary' }));
    await waitFor(() => {
      if (opened.length === 0) throw new Error('not yet');
    });
    expect(invoke).toHaveBeenCalledWith('repo_clone', {
      request: {
        url: 'git@gitlab.com:open-source-23/coral.git',
        parent: '/home/someone/Work',
        name: null,
        sshKey: null,
        depth: null,
        blobless: false,
      },
    });
  });

  it('shows what git said about a clone that exited 0 and checked nothing out', async () => {
    // git warns "remote HEAD refers to nonexistent ref, unable to checkout" and exits 0. What
    // arrives is a directory with a .git in it and no files, and the window opened it without
    // a word.
    const { view, opened } = await page({
      repo_clone: {
        at: '/home/someone/Work/cloned',
        notes: ['warning: remote HEAD refers to nonexistent ref, unable to checkout'],
      },
    });
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'git@gitlab.com:open-source-23/coral.git' },
    });
    await fireEvent.click(view.getByText('Choose…'));
    await waitFor(() => {
      if (!view.container.textContent?.includes('/home/someone/Work/coral')) {
        throw new Error('not yet');
      }
    });

    await fireEvent.click(view.getByText('Clone', { selector: '.primary' }));
    await waitFor(() => {
      if (opened.length === 0) throw new Error('not yet');
    });
    expect(view.container.querySelector('.notice')?.textContent).toContain('unable to checkout');
  });

  it('keeps a repository that fails to be made off the list of what was opened', async () => {
    const { view, opened } = await page({
      repo_init: new Error('there is already a git repository at /home/someone/Work/made'),
    });
    await fireEvent.click(view.getByText('Create'));
    await fireEvent.click(view.getByText('Choose…'));
    await fireEvent.input(view.getByPlaceholderText("the repository's name"), {
      target: { value: 'made' },
    });
    await fireEvent.click(view.getByText('Create', { selector: '.primary' }));

    await waitFor(() => {
      if (!view.container.textContent?.includes('already a git repository')) {
        throw new Error('not yet');
      }
    });
    expect(opened).toEqual([]);
  });

  it('takes one off the list without touching the repository', async () => {
    const { view } = await page();
    await fireEvent.click(view.getByTitle('Take alpha off this list'));

    await waitFor(() => {
      if (view.container.querySelectorAll('.repo-name').length !== 1) throw new Error('not yet');
    });
    expect(invoke).toHaveBeenCalledWith('forget_recent', {
      path: '/home/someone/Work/alpha',
    });
  });
});

describe('choosing which ssh key clones', () => {
  it('offers the keys only where one would be used', async () => {
    // An https clone authenticates through the credential helper and ignores ssh entirely. A
    // control that does nothing is worse than one that is not there.
    const { view } = await page();
    await fireEvent.click(view.getByText('Clone'));
    const url = view.getByPlaceholderText('https://host/team/thing.git');

    await fireEvent.input(url, { target: { value: 'https://host/team/thing.git' } });
    expect(view.container.querySelector('.sshkey'), 'not for https').toBeNull();

    await fireEvent.input(url, { target: { value: 'git@gitlab.com:team/thing.git' } });
    await waitFor(() => {
      if (!view.container.querySelector('.sshkey')) throw new Error('no key row yet');
    });

    await fireEvent.input(url, { target: { value: 'ssh://git@host:2222/team/thing.git' } });
    expect(view.container.querySelector('.sshkey'), 'and for an ssh URL').not.toBeNull();
  });

  it('sends the chosen key, and the agent when none is chosen', async () => {
    const { view, opened } = await page();
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'git@host:team/thing.git' },
    });
    await fireEvent.click(view.getByText('Choose…'));
    await waitFor(() => {
      if (!view.container.querySelector('.sshkey')) throw new Error('no key row yet');
    });

    const picker = view.container.querySelector('.sshkey select') as HTMLSelectElement;
    picker.value = '/home/someone/.ssh/id_work';
    await fireEvent.change(picker);
    await fireEvent.click(view.getByText('Clone', { selector: '.primary' }));
    await waitFor(() => {
      if (opened.length === 0) throw new Error('not yet');
    });

    expect(invoke).toHaveBeenCalledWith('repo_clone', {
      request: {
        url: 'git@host:team/thing.git',
        parent: '/home/someone/Work',
        name: null,
        sshKey: '/home/someone/.ssh/id_work',
        depth: null,
        blobless: false,
      },
    });
  });

  it('says that a passphrase cannot be asked for', async () => {
    // The engine pins GIT_TERMINAL_PROMPT=0 and SSH_ASKPASS_REQUIRE=never, so a key with a
    // passphrase and no agent fails rather than prompting. Silently is the worst way to learn
    // that.
    const { view } = await page();
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'git@host:team/thing.git' },
    });
    await waitFor(() => {
      if (!view.container.querySelector('.sshkey')) throw new Error('no key row yet');
    });
    expect(view.container.querySelector('.form')?.textContent).toContain('passphrase');
  });
});

describe('how much of a repository to take', () => {
  it('takes everything unless asked otherwise', async () => {
    const { view, opened } = await page();
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'https://host/team/thing.git' },
    });
    await fireEvent.click(view.getByText('Choose…'));
    await waitFor(() => {
      if (!view.container.textContent?.includes('/home/someone/Work/thing')) {
        throw new Error('the destination is not settled');
      }
    });
    await fireEvent.click(view.getByText('Clone', { selector: '.primary' }));
    await waitFor(() => {
      if (opened.length === 0) throw new Error('not yet');
    });

    const sent = invoke.mock.calls.filter((c) => c[0] === 'repo_clone').at(-1)?.[1];
    expect(sent).toMatchObject({ request: { depth: null, blobless: false } });
  });

  it('sends a depth for a shallow clone, and only then', async () => {
    // The two are different economies: shallow cuts the history off, partial keeps all of it
    // and leaves the file contents behind. Sending both would ask for something nobody chose.
    const { view, opened } = await page();
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'https://host/team/thing.git' },
    });
    await fireEvent.click(view.getByText('Choose…'));
    await waitFor(() => {
      if (!view.container.textContent?.includes('/home/someone/Work/thing')) {
        throw new Error('the destination is not settled');
      }
    });

    const take = view.container.querySelector('.history select') as HTMLSelectElement;
    take.value = 'shallow';
    await fireEvent.change(take);
    const depth = view.container.querySelector('.depth') as HTMLInputElement;
    depth.value = '50';
    await fireEvent.change(depth);

    await fireEvent.click(view.getByText('Clone', { selector: '.primary' }));
    await waitFor(() => {
      if (opened.length === 0) throw new Error('not yet');
    });
    expect(invoke.mock.calls.filter((c) => c[0] === 'repo_clone').at(-1)?.[1]).toMatchObject({
      request: { depth: 50, blobless: false },
    });
  });

  it('asks for a filter rather than a depth for a partial clone', async () => {
    const { view, opened } = await page();
    await fireEvent.click(view.getByText('Clone'));
    await fireEvent.input(view.getByPlaceholderText('https://host/team/thing.git'), {
      target: { value: 'https://host/team/thing.git' },
    });
    await fireEvent.click(view.getByText('Choose…'));
    await waitFor(() => {
      if (!view.container.textContent?.includes('/home/someone/Work/thing')) {
        throw new Error('the destination is not settled');
      }
    });

    const take = view.container.querySelector('.history select') as HTMLSelectElement;
    take.value = 'blobless';
    await fireEvent.change(take);
    expect(view.container.querySelector('.depth'), 'no depth to give').toBeNull();

    await fireEvent.click(view.getByText('Clone', { selector: '.primary' }));
    await waitFor(() => {
      if (opened.length === 0) throw new Error('not yet');
    });
    expect(invoke.mock.calls.filter((c) => c[0] === 'repo_clone').at(-1)?.[1]).toMatchObject({
      request: { depth: null, blobless: true },
    });
  });
});

describe('the directory a clone or a new repository lands in', () => {
  /**
   * It was read-only, so a path on the clipboard could not be pasted and a field that looked
   * like somewhere to type silently was not. The Choose… dialog is still there for anyone who
   * would rather browse.
   */
  it('can be typed as well as chosen', async () => {
    const { view } = await page();
    await fireEvent.click(
      [...view.container.querySelectorAll('button.action')].find(
        (b) => b.textContent?.includes('Clone'),
      ) as HTMLButtonElement,
    );

    const into = [...view.container.querySelectorAll('.form label')]
      .find((l) => l.textContent?.includes('Into'))
      ?.querySelector('input') as HTMLInputElement;
    expect(into, 'the form has a directory field').toBeTruthy();
    expect(into.readOnly, 'and it is not read-only').toBe(false);

    await fireEvent.input(into, { target: { value: '/home/dev/work' } });
    expect(into.value).toBe('/home/dev/work');
  });

  it('says both ways in the placeholder, so neither is hidden', async () => {
    const { view } = await page();
    await fireEvent.click(
      [...view.container.querySelectorAll('button.action')].find(
        (b) => b.textContent?.includes('Create'),
      ) as HTMLButtonElement,
    );
    const into = [...view.container.querySelectorAll('.form label')]
      .find((l) => l.querySelector('.name')?.textContent?.trim() === 'In')
      ?.querySelector('input') as HTMLInputElement;
    expect(into.placeholder).toBe('type or choose a directory');
    expect(into.readOnly).toBe(false);
  });
});
