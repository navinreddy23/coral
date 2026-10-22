// @vitest-environment happy-dom
import { cleanup, render, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
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

  it('fetches nothing again for the same commit handed over as new objects', async () => {
    // The diff is read again on every change in the working tree, and hands over fresh
    // objects each time; a commit's images cannot have changed.
    answer({ parent: 10, commit: 10 });
    const view = render(ImageDiff, { props: { repo: '/repo', before, after, stamp: 1 } });
    await waitFor(() => expect(view.container.querySelectorAll('img')).toHaveLength(2));
    await view.rerender({ repo: '/repo', before: { ...before }, after: { ...after }, stamp: 2 });
    await tick();
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(view.container.querySelectorAll('img')).toHaveLength(2);
  });

  it('reads a side on disk again when the diff is reread, keeping the old one up meanwhile', async () => {
    const disk = { side: { kind: 'worktree' } as const, file: 'img/logo.png' };
    const index = { side: { kind: 'index' } as const, file: 'img/logo.png' };
    answer({ parent: 10, commit: 10 });
    invoke.mockImplementation(async () => new Uint8Array(10).buffer);
    const view = render(ImageDiff, { props: { repo: '/repo', before: index, after: disk, stamp: 1 } });
    await waitFor(() => expect(view.container.querySelectorAll('img')).toHaveLength(2));
    const waiting: ((v: unknown) => void)[] = [];
    invoke.mockImplementation(() => new Promise((r) => waiting.push(r)));
    await view.rerender({ repo: '/repo', before: index, after: disk, stamp: 2 });
    await tick();
    expect(view.container.querySelectorAll('img'), 'still the old pictures').toHaveLength(2);
    expect(view.container.textContent).not.toContain('Loading');
    for (const r of waiting) r(new Uint8Array(12).buffer);
    await waitFor(() => expect(made.length).toBe(4));
    // The two replaced are let go once the new pair is up.
    await waitFor(() => expect(revoked.length).toBe(2));
  });

  it('lets go of the side that read when the other is refused', async () => {
    invoke.mockImplementation(async (_cmd: string, args: { side: { kind: string } }) => {
      if (args.side.kind === 'commit') throw { code: 'refused', message: 'it is 40 MB' };
      return new Uint8Array(10).buffer;
    });
    const { container } = render(ImageDiff, { props: { repo: '/repo', before, after } });
    await waitFor(() => expect(container.textContent).toContain('40 MB'));
    expect(made).toHaveLength(1);
    expect(revoked).toEqual(made);
  });
});
