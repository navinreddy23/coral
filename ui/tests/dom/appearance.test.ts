// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import Appearance from '../../src/app/Appearance.svelte';
import { ThemeState } from '../../src/state/theme.svelte';
import { ViewsState } from '../../src/state/views.svelte';

afterEach(cleanup);

/** The desktop's answer, and a way to change it while the pane is open. */
let listeners: ((event: { matches: boolean }) => void)[] = [];

function desktop(dark: boolean) {
  listeners = [];
  vi.stubGlobal('window', {
    matchMedia: () => ({
      matches: dark,
      addEventListener: (_: string, fn: (e: { matches: boolean }) => void) => listeners.push(fn),
      removeEventListener: () => {},
    }),
  });
}

function pane(dark = false) {
  desktop(dark);
  const theme = new ThemeState();
  const views = new ViewsState();
  const view = render(Appearance, { props: { theme, views } });
  return { view, theme, views };
}

function chosen(container: HTMLElement): string {
  return container.querySelector('.choice.on .label')?.textContent?.trim() ?? '';
}

describe('the appearance pane', () => {
  beforeEach(() => {
    vi.unstubAllGlobals();
    localStorage.clear();
  });

  it('opens on the desktop, because that is the shipped choice', () => {
    const { view } = pane();
    expect(chosen(view.container)).toBe('Follow the desktop');
  });

  it('offers all three at once rather than behind a dropdown', () => {
    const { view } = pane();
    const labels = [...view.container.querySelectorAll('.choice .label')].map((l) =>
      l.textContent?.trim(),
    );
    expect(labels).toEqual([
      'Follow the desktop', 'Light', 'Dark',
      'Compact', 'Default', 'Comfortable',
    ]);
  });

  it('switches the window when a choice is pressed', async () => {
    const { view, theme } = pane();
    await fireEvent.click(view.getByText('Dark'));

    expect(theme.choice).toBe('dark');
    expect(theme.current).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('hands the choice back to the desktop', async () => {
    const { view, theme } = pane(true);
    await fireEvent.click(view.getByText('Light'));
    expect(theme.current).toBe('light');

    await fireEvent.click(view.getByText('Follow the desktop'));
    expect(theme.following).toBe(true);
    expect(theme.current, 'the desktop is dark').toBe('dark');
  });

  it('says which way the desktop is leaning, while it is the one deciding', () => {
    const { view } = pane(true);
    expect(view.container.textContent).toContain('currently asking for dark');
  });

  it('says nothing about the desktop once it has been overruled', async () => {
    // The line exists to explain a state the window is in. Left up after a choice it would
    // be describing something that is no longer happening.
    const { view } = pane(true);
    await fireEvent.click(view.getByText('Light'));
    expect(view.container.textContent).not.toContain('currently asking for');
  });

  it('marks the choice for a screen reader, not only with a colour', () => {
    const { view } = pane();
    const pressed = [...view.container.querySelectorAll('.choice')].map((c) =>
      c.getAttribute('aria-pressed'),
    );
    expect(pressed).toEqual(['true', 'false', 'false', 'false', 'true', 'false']);
  });

  it('opens on the shipped density, which is the dense one', () => {
    const { views } = pane();
    expect(views.current.density).toBe('default');
  });

  it('sets the density on the root element, so every list follows it', async () => {
    // Written where `data-theme` is written, and for the same reason: the graph is not the
    // only list in the window.
    const { view, views } = pane();
    await fireEvent.click(view.getByText('Compact'));
    expect(views.current.density).toBe('compact');
  });

  it('draws each density rather than naming a number of pixels', () => {
    // Three rules with the gap between them carrying the difference. Nobody knows what
    // twenty-four pixels looks like; everybody can see which of three is tighter.
    const { view } = pane();
    expect(view.container.querySelectorAll('.choice .rows .rule')).toHaveLength(9);
    const gaps = [...view.container.querySelectorAll('.choice .rows')].map((r) =>
      Number.parseFloat((r as HTMLElement).style.gap),
    );
    expect(gaps).toHaveLength(3);
    expect(gaps[0]).toBeLessThan(gaps[1] ?? 0);
    expect(gaps[1]).toBeLessThan(gaps[2] ?? 0);
  });
});
