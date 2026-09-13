// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
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
      // Stated, not inferred from the placeholder: a question that wants text but has no hint
      // to offer used to render no field at all.
      asksText: true,
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

  it('asks for a box when more than one line is wanted, and keeps every line', async () => {
    // A commit message is a summary and a body. Asked for in a single line, the body could be
    // neither read nor kept: rewording any commit that had one threw it away.
    const { answers, container } = ask({ lines: 10, initial: 'summary\n\nbody' });
    const box = container.querySelector('textarea') as HTMLTextAreaElement;
    expect(box, 'a box, not a line').not.toBeNull();
    expect(container.querySelector('input')).toBeNull();
    expect(box.value).toBe('summary\n\nbody');

    await fireEvent.input(box, { target: { value: 'new summary\n\nnew body\nand more' } });
    await fireEvent.click(container.querySelector('button.primary') as HTMLButtonElement);
    expect(answers).toEqual([{ choice: 'create', text: 'new summary\n\nnew body\nand more' }]);
  });

  it('lets Enter be a newline in a box, and answers on Ctrl+Enter', async () => {
    const { answers } = ask({ lines: 10, initial: 'summary' });
    await fireEvent.keyDown(window, { key: 'Enter' });
    expect(answers, 'Enter types a line, it does not answer').toEqual([]);

    await fireEvent.keyDown(window, { key: 'Enter', ctrlKey: true });
    expect(answers).toEqual([{ choice: 'create', text: 'summary' }]);
  });

  it('still answers on a bare Enter when it asked for one line', async () => {
    const { answers } = ask({ initial: 'feature/x' });
    await fireEvent.keyDown(window, { key: 'Enter' });
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
      asksText: false,
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

describe('whether a question offers a field', () => {
  it('offers none unless it says it wants one', () => {
    // A confirmation is not a prompt. This was read off the placeholder, so a question that
    // wanted text but had no hint to offer silently became one that could only be cancelled.
    const { container } = ask({ asksText: false, placeholder: '' });
    expect(container.querySelector('input')).toBeNull();
  });

  it('offers one even with no placeholder to put in it', () => {
    const { container } = ask({ asksText: true, placeholder: '' });
    expect(container.querySelector('input')).not.toBeNull();
  });

  it('lets a confirmation be taken, since there is nothing to type', () => {
    // The primary button is disabled while the text is empty. For a question with no field
    // that would disable it forever.
    const { container } = ask({ asksText: false, placeholder: '' });
    const primary = container.querySelector('button.primary') as HTMLButtonElement;
    expect(primary.disabled).toBe(false);
  });
});

describe('the button that destroys something', () => {
  /**
   * This dialog already refuses to give a destructive question a key that answers it: Enter is
   * pressed to dismiss things, and nothing that cannot be undone should be reachable that way.
   * The button itself said nothing, though — "Delete v2" and "Cancel" were the same grey pair,
   * in a window whose menus mark every destructive line in red.
   */
  it('is marked, where an ordinary one is not', () => {
    const { container } = ask({
      title: 'Delete the tag v2?',
      asksText: false,
      choices: [{ id: 'delete', label: 'Delete v2', danger: true }],
    });
    const button = [...container.querySelectorAll('.choices button')].find(
      (b) => b.textContent?.trim() === 'Delete v2',
    );
    expect(button?.classList.contains('danger')).toBe(true);
    expect(container.querySelector('button.cancel')?.classList.contains('danger')).toBe(false);
  });

  it('takes its colour from the token the rest of the window uses for this', () => {
    ask({ asksText: false, choices: [{ id: 'delete', label: 'Delete', danger: true }] });
    const rule = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText)
      .find((text) => /button\.danger/u.test(text));
    expect(rule).toMatch(/var\(--danger/u);
  });

  it('is still not the one Enter answers', () => {
    // Marking it must not quietly make it primary.
    const { answers } = ask({
      asksText: false,
      choices: [{ id: 'delete', label: 'Delete', danger: true }],
    });
    void fireEvent.keyDown(window, { key: 'Enter' });
    expect(answers).toEqual([]);
  });
});

describe('a question with more than one answer', () => {
  /**
   * Every answer that destroys something is marked, not only the single-answer case. Discarding
   * offers two — keep the new files, or delete them too — and going to a branch that is already
   * here offers a checkout beside a hard reset. In each pair the destructive ones were the same
   * grey as the safe one.
   */
  it('marks each of them on its own', () => {
    const { container } = ask({
      title: 'topic is already here',
      asksText: false,
      choices: [
        { id: 'checkout', label: 'Checkout topic', primary: true },
        { id: 'reset', label: 'Reset topic to origin/topic', danger: true },
      ],
    });
    const marked = [...container.querySelectorAll('.choices button')].map((b) => [
      b.textContent?.trim(),
      b.classList.contains('danger'),
    ]);
    expect(marked).toEqual([
      ['Cancel', false],
      ['Checkout topic', false],
      ['Reset topic to origin/topic', true],
    ]);
  });
});
