// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import NewRequest from '../../src/app/NewRequest.svelte';

function form(over: Record<string, unknown> = {}) {
  const onClose = vi.fn();
  const onOpened = vi.fn();
  return {
    onClose,
    onOpened,
    ...render(NewRequest, {
      props: {
        repo: '/repo',
        source: 'feature/thing',
        targets: ['develop', 'main', 'release'],
        label: 'Pull request',
        onClose,
        onOpened,
        ...over,
      },
    }),
  };
}

describe('opening a pull or merge request', () => {
  it('guesses the branch it would land on, and offers the rest', () => {
    const { container } = form();
    const select = container.querySelector('select') as HTMLSelectElement;
    // `main` over `develop`, which comes first alphabetically but is not where things land.
    expect(select.value).toBe('main');
    expect([...select.options].map((o) => o.value)).toEqual(['develop', 'main', 'release']);
  });

  it('starts the title as the branch name, which is right often enough to keep', () => {
    const { container } = form();
    expect((container.querySelector('input') as HTMLInputElement).value).toBe('feature/thing');
  });

  it('refuses to open one with no title', async () => {
    const { container } = form();
    const title = container.querySelector('input') as HTMLInputElement;
    const go = container.querySelector('button.go') as HTMLButtonElement;
    expect(go.disabled).toBe(false);

    title.value = '   ';
    await fireEvent.input(title);
    expect(go.disabled).toBe(true);
  });

  it('refuses to merge a branch into itself, and says why', async () => {
    const { container } = form({ targets: ['feature/thing', 'main'] });
    const select = container.querySelector('select') as HTMLSelectElement;
    select.value = 'feature/thing';
    await fireEvent.change(select);

    expect((container.querySelector('button.go') as HTMLButtonElement).disabled).toBe(true);
    expect(container.textContent).toContain('cannot be merged into itself');
  });

  it('says so when the remote has nothing to merge into', () => {
    const { container } = form({ targets: [] });
    expect(container.querySelector('select')).toBeNull();
    expect(container.textContent).toContain('no branch on the remote');
    expect((container.querySelector('button.go') as HTMLButtonElement).disabled).toBe(true);
  });

  it('sends what was filled in, and hands back what the host made', async () => {
    invoke.mockReset();
    invoke.mockResolvedValue({ number: 7, title: 'a change', url: '', state: 'open', author: '', draft: true });
    const { container, onOpened, onClose } = form();
    const body = container.querySelector('textarea') as HTMLTextAreaElement;
    body.value = 'why it matters';
    await fireEvent.input(body);
    await fireEvent.click(container.querySelector('input[type=checkbox]') as HTMLInputElement);
    await fireEvent.click(container.querySelector('button.go') as HTMLButtonElement);

    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalled();
    });
    expect(invoke).toHaveBeenCalledWith('hosting_create', {
      path: '/repo',
      title: 'feature/thing',
      body: 'why it matters',
      source: 'feature/thing',
      target: 'main',
      draft: true,
    });
    await vi.waitFor(() => {
      expect(onOpened).toHaveBeenCalled();
      expect(onClose).toHaveBeenCalled();
    });
  });

  it('keeps the form up and says what went wrong when the host refuses', async () => {
    invoke.mockReset();
    invoke.mockRejectedValue(new Error('no token stored for github.com'));
    const { container, onClose } = form();
    await fireEvent.click(container.querySelector('button.go') as HTMLButtonElement);

    await vi.waitFor(() => {
      expect(container.textContent).toContain('no token stored');
    });
    expect(onClose, 'the form stays up so the text is not lost').not.toHaveBeenCalled();
  });
});
