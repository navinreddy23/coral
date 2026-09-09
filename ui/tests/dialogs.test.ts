import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { open } from '@tauri-apps/plugin-dialog';
import { pickDirectory, pickPatchFiles } from '../src/ipc/commands';

const dialog = vi.mocked(open);
beforeEach(() => dialog.mockReset());

/**
 * Every one of these dialogs is about the repository that is open, and every one of them
 * opened wherever the process happened to start — which for somebody who launched Coral from
 * a terminal is that terminal's directory, and for everybody else is their home. Writing a
 * patch out of a repository meant navigating back to it first, every time.
 */
describe('where a picker starts', () => {
  it('starts in the place the caller names', async () => {
    dialog.mockResolvedValue('/home/dev/journal');
    await pickDirectory('Where should the patch be written?', '/home/dev/journal');
    expect(dialog.mock.calls[0]?.[0]).toMatchObject({ defaultPath: '/home/dev/journal' });
  });

  it('names no place when the caller has none to give', async () => {
    // Cloning happens before there is a repository to start in.
    dialog.mockResolvedValue(null);
    await pickDirectory('Where should the clone go?');
    expect(dialog.mock.calls[0]?.[0]).not.toHaveProperty('defaultPath');
  });

  it('starts the patch picker there too, and still sorts a series', async () => {
    dialog.mockResolvedValue([
      '/home/dev/journal/0002-second.patch',
      '/home/dev/journal/0001-first.patch',
    ]);
    const files = await pickPatchFiles('Choose patch files', '/home/dev/journal');
    expect(dialog.mock.calls[0]?.[0]).toMatchObject({ defaultPath: '/home/dev/journal' });
    expect(files).toEqual([
      '/home/dev/journal/0001-first.patch',
      '/home/dev/journal/0002-second.patch',
    ]);
  });
});
