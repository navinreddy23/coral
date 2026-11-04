// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import FolderPicker from '../../src/app/FolderPicker.svelte';

const TREE: Record<string, string[]> = {
  '': ['arch', 'drivers', 'kernel'],
  drivers: ['drivers/gpu', 'drivers/net'],
  'drivers/gpu': ['drivers/gpu/drm'],
};

describe('the folder picker', () => {
  it('reads a level at a time, and a ticked folder brings what is inside it', async () => {
    invoke.mockImplementation(async (cmd: string, args: { under: string | null }) =>
      cmd === 'repo_folders' ? (TREE[args.under ?? ''] ?? []) : null,
    );
    const applied: string[][] = [];
    const view = render(FolderPicker, {
      props: { repo: '/k', initial: ['kernel'], onApply: (f: string[]) => applied.push(f), onClose: () => {} },
    });
    await waitFor(() => expect(view.container.querySelectorAll('.tree li')).toHaveLength(3));
    // Only the top was read.
    expect(invoke.mock.calls.filter(([c]) => c === 'repo_folders')).toHaveLength(1);

    const row = (name: string) =>
      [...view.container.querySelectorAll('.tree li')].find(
        (li) => li.querySelector('.mono')?.textContent === name,
      ) as HTMLElement;
    await fireEvent.click(row('drivers').querySelector('.twist') as HTMLElement);
    await waitFor(() => expect(row('gpu')).toBeDefined());
    await fireEvent.click(row('drivers').querySelector('input') as HTMLElement);

    // gpu is in because drivers is, and says so by being ticked and fixed.
    const gpu = row('gpu').querySelector('input') as HTMLInputElement;
    expect(gpu.checked).toBe(true);
    expect(gpu.disabled).toBe(true);

    expect(view.container.querySelector('footer .muted')?.textContent).toContain('2 folders');
    await fireEvent.click(view.container.querySelector('footer .primary') as HTMLElement);
    expect(applied).toEqual([['drivers', 'kernel']]);
  });
});
