// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
const openUrl = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl }));

import About from '../../src/app/About.svelte';

const BUILD = { number: '1.2.3', commit: 'abc1234', debug: false };

function answering(update: unknown) {
  invoke.mockImplementation(async (command: string) => {
    if (command === 'app_version') return BUILD;
    if (command === 'check_for_update') {
      if (update instanceof Error) throw update;
      return update;
    }
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
  const view = render(About, { props: { onCopied: () => {} } });
  await waitFor(() => checkButton(view.container));
  return view;
}

describe('checking for a newer Coral', () => {
  beforeEach(() => {
    invoke.mockReset();
    openUrl.mockReset();
  });

  it('asks nothing until the button is pressed', async () => {
    // The window contacts nothing on its own; the website says so.
    answering({ current: '1.2.3', latest: '1.2.3', newer: false, url: 'https://x.invalid' });
    await mounted();
    expect(invoke.mock.calls.map((c) => c[0])).not.toContain('check_for_update');
  });

  it('names a newer release and opens its page', async () => {
    const url = 'https://github.com/navinreddy23/coral/releases/tag/v1.2.4';
    answering({ current: '1.2.3', latest: '1.2.4', newer: true, url });
    const { container } = await mounted();
    await fireEvent.click(checkButton(container));
    await waitFor(() => {
      if (!container.textContent?.includes('Coral 1.2.4 is out')) throw new Error('not yet');
    });
    const open = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('release page'),
    );
    expect(open).toBeDefined();
    await fireEvent.click(open as HTMLButtonElement);
    expect(openUrl).toHaveBeenCalledWith(url);
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
