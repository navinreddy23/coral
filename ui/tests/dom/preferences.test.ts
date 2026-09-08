// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import Preferences from '../../src/app/Preferences.svelte';
import { ExperimentalState } from '../../src/state/experimental.svelte';
import { ProfilesState } from '../../src/state/profiles.svelte';
import { SigningState } from '../../src/state/signing.svelte';
import { SshState } from '../../src/state/ssh.svelte';
import { ThemeState } from '../../src/state/theme.svelte';
import { ViewsState } from '../../src/state/views.svelte';

afterEach(cleanup);

function prefs(over: Record<string, unknown> = {}) {
  const onClose = vi.fn();
  return {
    onClose,
    ...render(Preferences, {
      props: {
        signing: new SigningState(),
        ssh: new SshState(),
        experimental: new ExperimentalState(),
        profiles: new ProfilesState(),
        theme: new ThemeState(),
        views: new ViewsState(),
        identity: null,
        pane: null,
        repository: '/home/dev/coral',
        hasRepository: true,
        onClose,
        onCopied: vi.fn(),
        onPickGit: vi.fn(),
        onSwitchProfile: vi.fn(),
        onDeleteProfile: vi.fn(),
        onApplyProfileHere: vi.fn(),
        ...over,
      },
    }),
  };
}

describe('the way out of preferences', () => {
  /**
   * This panel covers the whole window, so leaving it is the one thing somebody who opened it
   * by mistake needs, and the only way out used to be a text link at the top of the side nav —
   * the same size and shape as the pane buttons under it, in the corner nobody looks in.
   */
  it('offers it in the header, where every other panel that covers the window puts one', () => {
    const { container } = prefs();
    const shut = container.querySelector('header button.shut');
    expect(shut, 'a close control in the header').not.toBeNull();
    expect(shut?.textContent?.trim(), 'the word as well as the glyph').toContain('Close');
    expect(shut?.querySelector('svg'), 'and the glyph as well as the word').not.toBeNull();
  });

  it('says which key does the same thing', () => {
    const title = prefs().container.querySelector('header button.shut')?.getAttribute('title');
    expect(title).toContain('Esc');
  });

  it('closes when it is pressed, and when Escape is', async () => {
    const { container, onClose } = prefs();
    await fireEvent.click(container.querySelector('header button.shut') as HTMLButtonElement);
    expect(onClose).toHaveBeenCalledTimes(1);

    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('leaves the pane list as a list of panes, with nothing in it that leaves', () => {
    // The link used to sit above them and read as one more thing to open.
    const { container } = prefs();
    const panes = [...container.querySelectorAll('nav button')].map((b) => b.textContent?.trim());
    expect(panes.length).toBeGreaterThan(3);
    expect(panes.some((p) => p?.includes('Close'))).toBe(false);
  });

  it('names the repository beside the title, for the panes that are about one', () => {
    // Two panes here describe one repository, and with two open an ssh key on screen says
    // nothing about whose it is.
    const { container } = prefs();
    expect(container.querySelector('header .repo')?.textContent).toBe('coral');
  });

  it('paints the header, because it carries text', () => {
    // A surface with no background of its own drops to grayscale antialiasing under WebKit.
    const rule = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText)
      .find((text) => /header[^{]*\{/u.test(text) && /background:/u.test(text));
    expect(rule).toBeDefined();
  });
});
