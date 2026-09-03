import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ThemeState } from '../src/state/theme.svelte';

/** A minimal document and storage, so the theme can be tested without a browser. */
function stub(stored: string | null = null) {
  const attrs = new Map<string, string>();
  vi.stubGlobal('document', {
    documentElement: {
      setAttribute: (k: string, v: string) => attrs.set(k, v),
      removeAttribute: (k: string) => attrs.delete(k),
      getAttribute: (k: string) => attrs.get(k) ?? null,
    },
  });
  const store = new Map<string, string>();
  if (stored !== null) store.set('coral.theme', stored);
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => store.set(k, v),
  });
  return { attrs, store };
}

describe('ThemeState', () => {
  beforeEach(() => vi.unstubAllGlobals());

  it('defaults to light and sets no attribute', () => {
    const { attrs } = stub();
    const theme = new ThemeState();

    expect(theme.current).toBe('light');
    expect(attrs.has('data-theme')).toBe(false);
  });

  it('marks the root when switched to dark, and remembers it', () => {
    const { attrs, store } = stub();
    const theme = new ThemeState();

    theme.toggle();
    expect(theme.current).toBe('dark');
    expect(attrs.get('data-theme')).toBe('dark');
    expect(store.get('coral.theme')).toBe('dark');

    theme.toggle();
    expect(attrs.has('data-theme')).toBe(false);
  });

  it('restores a stored preference', () => {
    const { attrs } = stub('dark');
    const theme = new ThemeState();

    expect(theme.current).toBe('dark');
    expect(attrs.get('data-theme')).toBe('dark');
  });

  it('ignores a stored value that is not a theme', () => {
    stub('chartreuse');
    expect(new ThemeState().current).toBe('light');
  });

  it('still opens when storage throws', () => {
    vi.stubGlobal('document', {
      documentElement: { setAttribute: () => {}, removeAttribute: () => {} },
    });
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('storage disabled');
      },
      setItem: () => {
        throw new Error('storage disabled');
      },
    });

    const theme = new ThemeState();
    expect(theme.current).toBe('light');
    expect(() => theme.toggle()).not.toThrow();
  });
});
