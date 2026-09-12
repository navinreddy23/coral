// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * The start page over a tab whose repository would not open.
 *
 * The banner says what went wrong with the repository in the tab. Over the page that offers
 * repositories to open it reads as a complaint about that list instead.
 */
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import App from '../../src/app/App.svelte';

const REPO = '/not/a/repository';
const SESSION = {
  tabs: [{ id: 1, path: REPO, submodule: null, group: null, missing: false }],
  active: 1,
  groups: [],
};
const REFUSED = `not a git repository: ${REPO}`;

function answers(): Record<string, unknown> {
  return {
    initial_repo: REPO,
    open_repo: new Error(REFUSED),
    session_get: SESSION,
    tab_open: SESSION,
    tab_activate: SESSION,
    binary_self_test: Uint8Array.from({ length: 4096 }, (_, i) => i % 251).buffer,
    recent_repos: [
      { path: '/home/someone/alpha', name: 'alpha', opened: 1_756_000_000, missing: false },
    ],
    ssh_keys: [],
    clone_history: [],
    lfs_available: true,
  };
}

beforeEach(() => {
  localStorage.clear();
  invoke.mockReset();
});

describe('the start page over a repository that would not open', () => {
  it('leaves the tab’s error behind, and shows it again on the way back', async () => {
    const table = answers();
    invoke.mockImplementation(async (cmd: string) => {
      if (!(cmd in table)) throw new Error(`unstubbed command ${cmd}`);
      const answer = table[cmd];
      if (answer instanceof Error) throw answer;
      return answer;
    });
    const { container, getByTitle } = render(App);

    await waitFor(() => {
      if (!container.querySelector('.banner.error')) throw new Error('no banner yet');
    });
    expect(container.querySelector('.banner.error')?.textContent).toContain(REFUSED);

    await fireEvent.click(getByTitle('Open a repository'));
    await waitFor(() => {
      if (!container.querySelector('.recents')) throw new Error('not on the start page yet');
    });
    expect(container.querySelector('.banner.error')).toBeNull();

    // Back to the tab: the reason it is empty has to be on screen again.
    const shut = container.querySelector('.tab.new .shut');
    if (shut === null) throw new Error('the new tab has no way back');
    await fireEvent.click(shut);
    await waitFor(() => {
      if (!container.querySelector('.banner.error')) throw new Error('banner did not come back');
    });
  });
});
