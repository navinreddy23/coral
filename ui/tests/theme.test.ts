import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ThemeState } from '../src/state/theme.svelte';

/** Listeners the fake media query is holding, so a test can fire a desktop change. */
let listeners: ((event: { matches: boolean }) => void)[] = [];

/**
 * A minimal document, storage and media query, so the theme can be tested without a browser.
 *
 * `dark` is what the desktop is asking for; `stored` is what the window was told, if anything.
 */
function stub({ dark = false, stored = null as string | null, media = true } = {}) {
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
  listeners = [];
  vi.stubGlobal('window', {
    matchMedia: media
      ? () => ({
          matches: dark,
          addEventListener: (_: string, fn: (e: { matches: boolean }) => void) =>
            listeners.push(fn),
          removeEventListener: (_: string, fn: (e: { matches: boolean }) => void) => {
            listeners = listeners.filter((l) => l !== fn);
          },
        })
      : undefined,
  });
  return { attrs, store };
}

/** What a desktop switching theme while Coral is open looks like from in here. */
function desktopSwitchesTo(theme: 'light' | 'dark') {
  for (const listener of listeners) listener({ matches: theme === 'dark' });
}

describe('ThemeState', () => {
  beforeEach(() => vi.unstubAllGlobals());

  it('follows a light desktop when nobody has chosen', () => {
    const { attrs } = stub({ dark: false });
    const theme = new ThemeState();

    expect(theme.choice).toBe('system');
    expect(theme.current).toBe('light');
    expect(theme.following).toBe(true);
    expect(attrs.has('data-theme')).toBe(false);
  });

  it('follows a dark desktop when nobody has chosen', () => {
    // The reason this exists: Coral used to open light on a dark desktop and stay there.
    const { attrs } = stub({ dark: true });
    const theme = new ThemeState();

    expect(theme.current).toBe('dark');
    expect(attrs.get('data-theme')).toBe('dark');
  });

  it('keeps following after the window is open', () => {
    // A desktop that goes dark in the evening takes Coral with it, without a restart.
    const { attrs } = stub({ dark: false });
    const theme = new ThemeState();

    desktopSwitchesTo('dark');
    expect(theme.current).toBe('dark');
    expect(attrs.get('data-theme')).toBe('dark');

    desktopSwitchesTo('light');
    expect(attrs.has('data-theme')).toBe(false);
  });

  it('stops following once somebody chooses, and stays where it was put', () => {
    const { attrs, store } = stub({ dark: false });
    const theme = new ThemeState();

    theme.toggle();
    expect(theme.choice).toBe('dark');
    expect(theme.following).toBe(false);
    expect(store.get('coral.theme')).toBe('dark');

    desktopSwitchesTo('light');
    expect(theme.current, 'a choice outranks the desktop').toBe('dark');
    expect(attrs.get('data-theme')).toBe('dark');
  });

  it('goes back to following when it is asked to', () => {
    const { attrs, store } = stub({ dark: true, stored: 'light' });
    const theme = new ThemeState();
    expect(theme.current).toBe('light');

    theme.set('system');
    expect(theme.current).toBe('dark');
    expect(attrs.get('data-theme')).toBe('dark');
    expect(store.get('coral.theme')).toBe('system');
  });

  it('honours a theme stored before there was anything else to store', () => {
    // Nothing ever wrote a bare theme there but somebody pressing the switch, so an upgrade
    // must not take that away and start following the desktop instead.
    stub({ dark: true, stored: 'light' });
    expect(new ThemeState().current).toBe('light');
  });

  it('ignores a stored value that is not a choice', () => {
    stub({ dark: false, stored: 'chartreuse' });
    expect(new ThemeState().choice).toBe('system');
  });

  it('toggles away from what is showing, not from what was chosen', () => {
    // Following a dark desktop, the switch has to produce light. Flipping the *choice* would
    // turn `system` into `light` and change nothing on screen.
    stub({ dark: true });
    const theme = new ThemeState();
    expect(theme.current).toBe('dark');

    theme.toggle();
    expect(theme.current).toBe('light');
  });

  it('opens on a webview with no media query at all', () => {
    stub({ media: false });
    const theme = new ThemeState();
    expect(theme.current).toBe('light');
    expect(() => theme.toggle()).not.toThrow();
  });

  it('still opens when storage throws', () => {
    stub({ dark: false });
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

  it('lets go of the desktop when it is released', () => {
    stub({ dark: false });
    const theme = new ThemeState();
    theme.release();

    desktopSwitchesTo('dark');
    expect(theme.current).toBe('light');
  });
});
