// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke }));

const { ProfilesState } = await import('../src/state/profiles.svelte');

const BLANK = {
  user: { name: null, email: null },
  ssh: { privateKey: null, publicKey: null, credentialHelper: null },
  signing: { format: null, program: null, key: null, signCommits: null, signTags: null },
};

function registry(current = 'personal') {
  return {
    current,
    profiles: [
      { id: 'personal', name: 'Personal', colour: 'lane1', settings: BLANK },
      { id: 'work', name: 'Work', colour: 'lane3', settings: BLANK },
    ],
  };
}

beforeEach(() => {
  invoke.mockReset();
});

describe('which profile the window is in', () => {
  it('names one before any answer has arrived', () => {
    // The chip in the title strip renders on this. A window that opened with no name on it
    // would read as broken rather than as still loading.
    const state = new ProfilesState();
    expect(state.current.name).not.toBe('');
    expect(state.all.length).toBe(1);
  });

  it('follows the current id, not the order of the list', async () => {
    invoke.mockResolvedValue(registry('work'));
    const state = new ProfilesState();
    await state.load();

    expect(state.current.id).toBe('work');
    expect(state.current.name).toBe('Work');
  });

  it('falls back to the first when the current one names nothing', async () => {
    // The registry is repaired in Rust, but a window that renders on nothing at all would be
    // a blank chip rather than a wrong one, and this is what it renders on.
    invoke.mockResolvedValue(registry('deleted'));
    const state = new ProfilesState();
    await state.load();

    expect(state.current.id).toBe('personal');
  });

  it('answers with the workspace a switch brings, so the caller can put the old one away', async () => {
    const session = { tabs: [{ id: 7, path: '/srv/work', group: null }], groups: [], active: 7 };
    invoke.mockResolvedValue({ registry: registry('work'), session, recents: [] });

    const state = new ProfilesState();
    const answer = await state.switchTo('work');

    expect(invoke).toHaveBeenCalledWith('profile_switch', { id: 'work' });
    expect(answer?.session.tabs[0]?.path).toBe('/srv/work');
    expect(state.currentId, 'and the registry is taken from the same answer').toBe('work');
  });

  it('keeps the profile it had when a switch fails', async () => {
    // Half a switch is the worst outcome: the tabs of one profile under the name of another.
    invoke.mockResolvedValueOnce(registry('personal'));
    const state = new ProfilesState();
    await state.load();

    invoke.mockRejectedValueOnce(new Error('nope'));
    const answer = await state.switchTo('work');

    expect(answer).toBeNull();
    expect(state.currentId).toBe('personal');
    expect(state.error).not.toBeNull();
    expect(state.busy, 'and it is not left looking busy').toBe(false);
  });
});
