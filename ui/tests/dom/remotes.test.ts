// @vitest-environment happy-dom
import { cleanup, render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import Remotes from '../../src/app/Remotes.svelte';
import { RemotesState } from '../../src/state/remotes.svelte';
import type { Remote } from '../../src/ipc/types';

afterEach(cleanup);

const TWO: Remote[] = [
  { name: 'mirror', fetchUrl: 'git@example.com:x/mirror.git', pushUrl: 'git@example.com:x/mirror.git' },
  { name: 'origin', fetchUrl: 'git@example.com:x/thing.git', pushUrl: 'git@example.com:x/thing.git' },
];

/** The panel with its list already read, as it is by the time anyone opens it. */
async function panel(list: Remote[], focus: string | null = null) {
  invoke.mockImplementation(async () => list);
  const remotes = new RemotesState();
  await remotes.load('/repo');
  const view = render(Remotes, {
    props: { remotes, focus, onClose: () => {}, onChanged: () => {} },
  });
  return { view, remotes };
}

function heading(container: HTMLElement): string {
  return container.querySelector('.form h3')?.textContent?.trim() ?? '';
}

describe('the remotes panel', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('opens on the first remote when it was not reached from one', async () => {
    // Reached from the section header it opened on nothing, and showed the line meant for a
    // repository with no remotes beside a list of two.
    const { view } = await panel(TWO);
    await waitFor(() => {
      if (heading(view.container) === '') throw new Error('nothing chosen yet');
    });
    expect(heading(view.container)).toBe('mirror');
    expect(view.container.textContent).not.toContain('No remotes are configured');
  });

  it('opens on the remote it was reached from', async () => {
    const { view } = await panel(TWO, 'origin');
    await waitFor(() => {
      if (heading(view.container) !== 'origin') throw new Error('not yet');
    });
  });

  it('offers the add form when the repository really has none', async () => {
    const { view } = await panel([]);
    await waitFor(() => {
      if (heading(view.container) !== 'New remote') throw new Error('not yet');
    });
  });

  it('never says there are none while some are listed', async () => {
    // The panel contradicting its own list is worse than saying nothing.
    const { view } = await panel(TWO);
    await fireEvent.click(view.getByText('+ Add a remote'));
    expect(view.container.textContent).not.toContain('No remotes are configured');
  });

  it('waits for the list before deciding, since an empty one means two things', async () => {
    // Deciding before the answer arrives opened the add form in repositories that had remotes.
    invoke.mockImplementation(async () => TWO);
    const remotes = new RemotesState();
    const view = render(Remotes, {
      props: { remotes, focus: null, onClose: () => {}, onChanged: () => {} },
    });
    expect(heading(view.container), 'nothing decided before the read').toBe('');

    await remotes.load('/repo');
    await waitFor(() => {
      if (heading(view.container) !== 'mirror') throw new Error('not yet');
    });
  });

  it('shows a remote its own name and fetch url, and the buttons that act on it', async () => {
    const { view } = await panel(TWO, 'origin');
    await waitFor(() => {
      if (heading(view.container) !== 'origin') throw new Error('not yet');
    });
    const inputs = [...view.container.querySelectorAll('.form input')] as HTMLInputElement[];
    expect(inputs[0]?.value).toBe('origin');
    expect(inputs[1]?.value).toBe('git@example.com:x/thing.git');
    expect(view.container.textContent).toContain('Prune gone branches');
    expect(view.container.textContent).toContain('Remove');
  });

  it('asks again before removing one', async () => {
    const { view } = await panel(TWO, 'origin');
    await waitFor(() => {
      if (heading(view.container) !== 'origin') throw new Error('not yet');
    });
    invoke.mockClear();
    await fireEvent.click(view.getByText('Remove'));
    expect(invoke, 'nothing sent on the first click').not.toHaveBeenCalled();
    expect(view.container.textContent).toContain('Really remove origin');
  });
});
