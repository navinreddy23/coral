// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import Toolbar from '../../src/app/Toolbar.svelte';

afterEach(cleanup);

/** The toolbar as it appears in a repository, with nothing running and nothing compared. */
function bar(overrides: Record<string, unknown> = {}) {
  return render(Toolbar, {
    props: {
      repo: 'coral',
      path: '/home/dev/coral',
      submodule: null,
      branch: 'master',
      busy: false,
      comparing: false,
      terminalOpen: false,
      onAction: vi.fn(),
      onLeaveSubmodule: vi.fn(),
      onPullMenu: vi.fn(),
      onPushMenu: vi.fn(),
      ...overrides,
    },
  });
}

function buttons(container: HTMLElement): HTMLButtonElement[] {
  return [...container.querySelectorAll('button.action')] as HTMLButtonElement[];
}

describe('the toolbar', () => {
  it('offers every action it used to, in the order it used to', () => {
    // The words are gone from the face of the buttons; nothing else about them is.
    const { container } = bar();
    expect(buttons(container).map((b) => b.getAttribute('aria-label'))).toEqual([
      'Undo', 'Redo', 'Fetch', 'Pull', 'Push', 'Branch', 'Stash', 'Pop', 'Patch', 'Terminal',
    ]);
  });

  it('gives every button a name a screen reader can read', () => {
    // This is now the only name they have, so it is not decoration.
    for (const button of buttons(bar().container)) {
      expect(button.getAttribute('aria-label')?.length ?? 0).toBeGreaterThan(0);
    }
  });

  it('names the action first in the tooltip, then what it does', () => {
    const { container } = bar();
    const fetch = buttons(container).find((b) => b.getAttribute('aria-label') === 'Fetch');
    expect(fetch?.title).toMatch(/^Fetch — /);
  });

  it('carries the keystroke in the tooltip where the action has one', () => {
    // Read from the same table the help overlay is built from, not spelled out twice.
    const { container } = bar();
    const named = (label: string) =>
      buttons(container).find((b) => b.getAttribute('aria-label') === label)?.title ?? '';
    expect(named('Undo')).toContain('(Ctrl Z)');
    expect(named('Branch')).toContain('(Ctrl B)');
    expect(named('Terminal')).toContain('(Ctrl `)');
    // Pull has no binding in the table, so it must not invent one.
    expect(named('Pull')).not.toContain('(');
  });

  it('says which of its two jobs the patch button will do', () => {
    expect(bar().container.querySelector('[aria-label="Patch"]')?.getAttribute('title'))
      .toContain('apply a patch file');
    const comparing = bar({ comparing: true });
    expect(comparing.container.querySelector('[aria-label="Patch"]')?.getAttribute('title'))
      .toContain('write the commits');
  });

  it('draws every action, so none of them is a typeface glyph', () => {
    for (const button of buttons(bar().container)) {
      expect(button.querySelector('svg'), button.getAttribute('aria-label') ?? '').not.toBeNull();
    }
  });

  it('separates the three groups with a rule rather than with space', () => {
    // Space is the first thing a narrow window takes away, and the groups have to survive it.
    expect(bar().container.querySelectorAll('.actions .rule')).toHaveLength(2);
  });

  it('offers a choice beside pull and push, and beside nothing else', () => {
    const { container } = bar();
    const carets = [...container.querySelectorAll('button.caret')].map((c) => c.getAttribute('title'));
    expect(carets).toEqual(['Choose how to pull', 'Choose what to push']);
  });

  it('keeps the terminal at the far end, away from the repository actions', () => {
    // It acts on the window, not on the repository, and sitting inside the run of git actions
    // is what made that unclear.
    const { container } = bar();
    expect(container.querySelector('.trailing [aria-label="Terminal"]')).not.toBeNull();
    expect(container.querySelector('.actions [aria-label="Terminal"]')).toBeNull();
  });

  it('says whether the terminal pane is showing', () => {
    expect(bar().container.querySelector('[aria-label="Terminal"]')?.getAttribute('aria-pressed'))
      .toBe('false');
    expect(bar({ terminalOpen: true }).container
      .querySelector('[aria-label="Terminal"]')?.getAttribute('aria-pressed')).toBe('true');
  });

  it('disables the repository actions while one is running, and leaves the terminal alone', () => {
    const { container } = bar({ busy: true });
    const named = (label: string) =>
      buttons(container).find((b) => b.getAttribute('aria-label') === label);
    expect(named('Push')?.disabled).toBe(true);
    expect(named('Terminal')?.disabled).toBe(false);
  });

  it('shows the way out of a submodule on the crumb it belongs to', () => {
    const { container } = bar({ submodule: 'vendor/thing' });
    expect(container.querySelector('.step.sub .value')?.textContent).toBe('thing');
    expect(container.querySelector('.step.sub .leave')).not.toBeNull();
  });
});
