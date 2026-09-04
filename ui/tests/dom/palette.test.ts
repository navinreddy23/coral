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

import Palette, { type Command } from '../../src/app/Palette.svelte';

function commands(ran: string[]): Command[] {
  return [
    { id: 'push', label: 'Push', group: 'Remote', run: () => ran.push('push') },
    { id: 'pull', label: 'Pull (fast-forward only)', group: 'Remote', run: () => ran.push('pull') },
    { id: 'co', label: 'Checkout branch', group: 'Branch', run: () => ran.push('co') },
    { id: 'stash', label: 'Stash changes', group: 'Stash', run: () => ran.push('stash') },
  ];
}

function labels(container: HTMLElement): string[] {
  return [...container.querySelectorAll('.label')].map((e) => e.textContent?.trim() ?? '');
}

describe('the command palette', () => {
  it('lists everything before anything is typed', () => {
    const { container } = render(Palette, { props: { commands: commands([]), onClose: () => {} } });
    expect(labels(container)).toHaveLength(4);
  });

  it('narrows to a subsequence match', async () => {
    const { container } = render(Palette, { props: { commands: commands([]), onClose: () => {} } });
    const input = container.querySelector('input') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: 'cob' } });
    expect(labels(container)).toEqual(['Checkout branch']);
  });

  it('says so rather than showing an empty list', async () => {
    const { container } = render(Palette, { props: { commands: commands([]), onClose: () => {} } });
    await fireEvent.input(container.querySelector('input') as HTMLInputElement, {
      target: { value: 'zzzz' },
    });
    expect(container.querySelector('.empty')?.textContent).toContain('No command matches');
  });

  it('runs the highlighted command on Enter and closes', async () => {
    const ran: string[] = [];
    let closed = false;
    const { container } = render(Palette, {
      props: { commands: commands(ran), onClose: () => (closed = true) },
    });
    const input = container.querySelector('input') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: 'stash' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    expect(ran).toEqual(['stash']);
    expect(closed).toBe(true);
  });

  it('moves the highlight with the arrow keys', async () => {
    const ran: string[] = [];
    const { container } = render(Palette, {
      props: { commands: commands(ran), onClose: () => {} },
    });
    const input = container.querySelector('input') as HTMLInputElement;
    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await fireEvent.keyDown(input, { key: 'Enter' });
    expect(ran).toEqual(['pull']);
  });

  it('closes on Escape without running anything', async () => {
    const ran: string[] = [];
    let closed = false;
    const { container } = render(Palette, {
      props: { commands: commands(ran), onClose: () => (closed = true) },
    });
    await fireEvent.keyDown(container.querySelector('input') as HTMLInputElement, { key: 'Escape' });
    expect(closed).toBe(true);
    expect(ran).toEqual([]);
  });

  it('starts back at the top when the query changes', async () => {
    const ran: string[] = [];
    const { container } = render(Palette, { props: { commands: commands(ran), onClose: () => {} } });
    const input = container.querySelector('input') as HTMLInputElement;
    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    // Otherwise the highlight points past the end of a shorter result list.
    await fireEvent.input(input, { target: { value: 'push' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    expect(ran).toEqual(['push']);
  });
});
