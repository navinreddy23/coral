// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { HostingState } from '../src/state/hosting.svelte';

const HOST = {
  kind: 'gitlab',
  origin: 'https://gitlab.com',
  owner: 'group',
  repo: 'thing',
};

beforeEach(() => {
  invoke.mockReset();
});

/** A repository whose host refuses the stored token. */
function rejecting() {
  invoke.mockImplementation((name: string) => {
    if (name === 'hosting_status') {
      return Promise.resolve({ host: HOST, detail: null, token: 'profile' });
    }
    if (name === 'hosting_pull_requests') {
      return Promise.reject(new Error('the host refused the request (401)'));
    }
    return Promise.resolve({ host: HOST, detail: null, token: 'none' });
  });
}

describe('the hosting panel', () => {
  it('forgets the refusal along with the token it was about', async () => {
    // Signing out left "the host refused the request (401)" up, about a token that was no
    // longer there to be refused.
    rejecting();
    const hosting = new HostingState();
    await hosting.load('/repo');
    expect(hosting.error).toContain('401');

    await hosting.signOut();
    expect(hosting.error).toBeNull();
    expect(hosting.view?.token).toBe('none');
  });
});
