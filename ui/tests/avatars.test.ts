// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import { AvatarState } from '../src/state/avatars.svelte';

beforeEach(() => {
  invoke.mockReset();
  localStorage.clear();
  vi.stubGlobal('createImageBitmap', async () => ({ width: 64, height: 64 }) as ImageBitmap);
  vi.stubGlobal('URL', { ...URL, createObjectURL: () => 'blob:face' });
});
afterEach(() => {
  vi.unstubAllGlobals();
});

const picture = () => new Uint8Array([1, 2, 3]).buffer;

describe('authors’ pictures', () => {
  it('asks once for each author however often they appear, and however their address is written', async () => {
    invoke.mockResolvedValue(picture());
    const avatars = new AvatarState();
    avatars.want(['ada@example.org', 'Ada@Example.org ', 'grace@example.org']);
    avatars.want(['ada@example.org']);
    await vi.waitFor(() => expect(avatars.version).toBeGreaterThan(1));
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(avatars.bitmap('ADA@example.org')).not.toBeNull();
    expect(avatars.url('grace@example.org')).toBe('blob:face');
  });

  it('keeps the initials for an author with no picture', async () => {
    invoke.mockResolvedValue(new ArrayBuffer(0));
    const avatars = new AvatarState();
    avatars.want(['nobody@example.org']);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 0));
    expect(avatars.bitmap('nobody@example.org')).toBeNull();
  });

  it('keeps the initials when the bytes are not a picture', async () => {
    invoke.mockResolvedValue(picture());
    vi.stubGlobal('createImageBitmap', async () => {
      throw new Error('not an image');
    });
    const avatars = new AvatarState();
    avatars.want(['odd@example.org']);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 0));
    expect(avatars.bitmap('odd@example.org')).toBeNull();
  });

  it('asks nobody when switched off, remembers that, and shows nothing it already had', async () => {
    invoke.mockResolvedValue(picture());
    const avatars = new AvatarState();
    avatars.want(['ada@example.org']);
    await vi.waitFor(() => expect(avatars.bitmap('ada@example.org')).not.toBeNull());

    avatars.setEnabled(false);
    expect(avatars.bitmap('ada@example.org')).toBeNull();
    avatars.want(['grace@example.org']);
    expect(invoke).toHaveBeenCalledTimes(1);

    const next = new AvatarState();
    expect(next.enabled).toBe(false);
  });

  it('is on until it is turned off', () => {
    expect(new AvatarState().enabled).toBe(true);
  });
});
