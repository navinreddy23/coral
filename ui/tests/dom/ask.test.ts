// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import Ask from '../../src/app/Ask.svelte';

type Answer = { choice: string | null; text: string };

function ask(over: Record<string, unknown> = {}) {
  const answers: Answer[] = [];
  const view = render(Ask, {
    props: {
      title: 'New branch',
      detail: 'Created at master and checked out.',
      placeholder: 'feature/…',
      initial: '',
      choices: [{ id: 'create', label: 'Create branch', primary: true }],
      onAnswer: (choice: string | null, text: string) => answers.push({ choice, text }),
      ...over,
    },
  });
  return { answers, ...view };
}

describe('asking a question in the window', () => {
  it('reports the choice and the trimmed text', async () => {
    const { answers, container } = ask();
    const input = container.querySelector('input') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: '  feature/x  ' } });
    await fireEvent.click(container.querySelector('button.primary') as HTMLButtonElement);
    expect(answers).toEqual([{ choice: 'create', text: 'feature/x' }]);
  });

  it('will not answer with an empty name', async () => {
    // The button is the only way to say yes, so disabling it is the whole guard.
    const { container } = ask();
    expect((container.querySelector('button.primary') as HTMLButtonElement).disabled).toBe(true);
  });

  it('takes the primary choice on Enter and dismisses on Escape', async () => {
    const { answers, container } = ask();
    await fireEvent.input(container.querySelector('input') as HTMLInputElement, {
      target: { value: 'x' },
    });
    await fireEvent.keyDown(window, { key: 'Enter' });
    expect(answers[0]).toEqual({ choice: 'create', text: 'x' });

    const second = ask();
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(second.answers.at(-1)?.choice).toBeNull();
  });

  it('offers a choice without a text field when none is asked for', () => {
    const { container } = ask({
      title: 'Bring feature into master?',
      placeholder: '',
      choices: [
        { id: 'merge', label: 'Merge', primary: true },
        { id: 'rebase', label: 'Rebase' },
      ],
    });
    expect(container.querySelector('input')).toBeNull();
    const labels = [...container.querySelectorAll('.choices button')].map((b) =>
      b.textContent?.trim(),
    );
    expect(labels).toEqual(['Cancel', 'Merge', 'Rebase']);
    // Nothing is typed, so nothing may be disabled.
    expect([...container.querySelectorAll('.choices button')].every((b) => !(b as HTMLButtonElement).disabled)).toBe(true);
  });

  it('dismisses when the backdrop is clicked', async () => {
    const { answers, container } = ask();
    await fireEvent.click(container.querySelector('.scrim') as HTMLElement);
    expect(answers.at(-1)?.choice).toBeNull();
  });
});
