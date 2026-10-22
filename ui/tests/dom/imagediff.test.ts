// @vitest-environment happy-dom
import { cleanup, render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import ImageDiff, { isImage } from '../../src/app/ImageDiff.svelte';

afterEach(cleanup);

const made: string[] = [];
const revoked: string[] = [];
beforeEach(() => {
  invoke.mockReset();
  made.length = 0;
  revoked.length = 0;
  let n = 0;
  vi.stubGlobal('URL', {
    ...URL,
    createObjectURL: () => {
      const url = `blob:test/${(n += 1)}`;
      made.push(url);
      return url;
    },
    revokeObjectURL: (url: string) => revoked.push(url),
  });
});

const before = { side: { kind: 'parent', rev: 'abc' } as const, file: 'img/logo.png' };
const after = { side: { kind: 'commit', rev: 'abc' } as const, file: 'img/logo.png' };

function answer(sides: { parent: number; commit: number }) {
  invoke.mockImplementation(async (_cmd: string, args: { side: { kind: string } }) => {
    const length = args.side.kind === 'parent' ? sides.parent : sides.commit;
    return new Uint8Array(length).buffer;
  });
}

describe('an image before and after', () => {
  it('knows which files it can draw', () => {
    expect(isImage('a/logo.PNG')).toBe(true);
    expect(isImage('photo.jpeg')).toBe(true);
    expect(isImage('font.woff2')).toBe(false);
    expect(isImage('Makefile')).toBe(false);
  });

  it('draws both sides with their sizes', async () => {
    answer({ parent: 2048, commit: 3 * 1024 * 1024 });
    const { container } = render(ImageDiff, { props: { repo: '/repo', before, after } });
    await waitFor(() => expect(container.querySelectorAll('img')).toHaveLength(2));
    const text = container.textContent ?? '';
    expect(text).toContain('Before');
    expect(text).toContain('2.0 KB');
    expect(text).toContain('3.0 MB');
    expect(invoke).toHaveBeenCalledWith('file_blob', { path: '/repo', side: before.side, file: 'img/logo.png' });
  });

  it('says a side was not there, rather than drawing an empty frame', async () => {
    answer({ parent: 0, commit: 10 });
    const { container } = render(ImageDiff, { props: { repo: '/repo', before, after } });
    await waitFor(() => expect(container.querySelectorAll('img')).toHaveLength(1));
    expect(container.textContent).toContain('Not there before this change');
  });

  it('says what failed', async () => {
    invoke.mockRejectedValue({ code: 'refused', message: 'cannot show the file: it is 40 MB' });
    const { container } = render(ImageDiff, { props: { repo: '/repo', before, after } });
    await waitFor(() => expect(container.textContent).toContain('40 MB'));
  });

  it('lets go of the images when it closes', async () => {
    answer({ parent: 10, commit: 10 });
    const { container, unmount } = render(ImageDiff, { props: { repo: '/repo', before, after } });
    await waitFor(() => expect(container.querySelectorAll('img')).toHaveLength(2));
    unmount();
    expect(revoked.sort()).toEqual([...made].sort());
  });
});
