// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
const openUrl = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));

import About from '../../src/app/About.svelte';
import { UpdateState } from '../../src/state/update.svelte';

const BUILD = { number: '1.2.3', commit: 'abc1234', debug: false };

function answering(update: unknown) {
  invoke.mockImplementation(async (command: string) => {
    if (command === 'app_version') return BUILD;
    if (command === 'check_for_update') {
      if (update instanceof Error) throw update;
      return update;
    }
    if (command === 'install_update') return undefined;
    throw new Error(`unexpected ${command}`);
  });
}

function checkButton(container: HTMLElement): HTMLButtonElement {
  const found = [...container.querySelectorAll('button')].find((b) =>
    b.textContent?.includes('Check for updates'),
  );
  if (!found) throw new Error('no check button');
  return found;
}

async function mounted() {
  const updates = new UpdateState();
  const view = render(About, { props: { onCopied: () => {}, updates } });
  await waitFor(() => checkButton(view.container));
  return { ...view, updates };
}

function named(container: HTMLElement, text: string): HTMLButtonElement | undefined {
  return [...container.querySelectorAll('button')].find((b) => b.textContent?.includes(text));
}

describe('checking for a newer Coral', () => {
  beforeEach(() => {
    invoke.mockReset();
    openUrl.mockReset();
    localStorage.clear();
  });

  it('asks nothing until the button is pressed', async () => {
    // The window contacts nothing on its own; the website says so.
    answering({ current: '1.2.3', latest: '1.2.3', newer: false, url: 'https://x.invalid' });
    await mounted();
    expect(invoke.mock.calls.map((c) => c[0])).not.toContain('check_for_update');
  });

  it('installs a newer release in place where this copy can be updated', async () => {
    const url = 'https://github.com/navinreddy23/coral/releases/tag/v1.2.4';
    answering({ current: '1.2.3', latest: '1.2.4', newer: true, url, installable: true });
    const { container } = await mounted();
    await fireEvent.click(checkButton(container));
    await waitFor(() => {
      if (!named(container, 'Install and restart')) throw new Error('no install button');
    });
    await fireEvent.click(named(container, 'Install and restart') as HTMLButtonElement);
    await waitFor(() => {
      if (!invoke.mock.calls.some((c) => c[0] === 'install_update')) throw new Error('not installed');
    });
  });

  it('names a newer release and opens its page where it cannot be installed in place', async () => {
    const url = 'https://github.com/navinreddy23/coral/releases/tag/v1.2.4';
    answering({ current: '1.2.3', latest: '1.2.4', newer: true, url, installable: false });
    const { container } = await mounted();
    await fireEvent.click(checkButton(container));
    await waitFor(() => {
      if (!container.textContent?.includes('Coral 1.2.4 is out')) throw new Error('not yet');
    });
    const open = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('release page'),
    );
    expect(open).toBeDefined();
    expect(named(container, 'Install and restart')).toBeUndefined();
    await fireEvent.click(open as HTMLButtonElement);
    expect(openUrl).toHaveBeenCalledWith(url);
  });

  it('turns the check at launch on and off, off to begin with', async () => {
    answering({ current: '1.2.3', latest: '1.2.3', newer: false, url: 'https://x.invalid', installable: false });
    const { container, updates } = await mounted();
    const box = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(box.checked).toBe(false);
    await fireEvent.click(box);
    expect(updates.automatic).toBe(true);
    expect(localStorage.getItem('coral.updates.automatic')).toBe('true');
  });

  it('says so when this is the newest release, and offers no page', async () => {
    answering({ current: '1.2.3', latest: '1.2.3', newer: false, url: 'https://x.invalid' });
    const { container } = await mounted();
    await fireEvent.click(checkButton(container));
    await waitFor(() => {
      if (!container.textContent?.includes('newest release')) throw new Error('not yet');
    });
    expect(container.textContent).not.toContain('release page');
  });

  it('does not offer an older release to a build that is ahead of it', async () => {
    answering({ current: '1.3.0', latest: '1.2.4', newer: false, url: 'https://x.invalid' });
    const { container } = await mounted();
    await fireEvent.click(checkButton(container));
    await waitFor(() => {
      if (!container.textContent?.includes('ahead of')) throw new Error('not yet');
    });
    expect(container.textContent).toContain('1.2.4');
    expect(container.textContent).not.toContain('release page');
  });

  it('reports a check that could not reach GitHub, and can be asked again', async () => {
    answering(new Error('could not reach the host: dns'));
    const { container } = await mounted();
    await fireEvent.click(checkButton(container));
    await waitFor(() => {
      if (!container.textContent?.includes('could not reach')) throw new Error('not yet');
    });
    expect(checkButton(container).disabled).toBe(false);
  });
});
