// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));

import Grep from '../../src/app/Grep.svelte';
import { GrepState } from '../../src/state/grep.svelte';
import type { GrepMatch } from '../../src/ipc/types';

function shown(matches: GrepMatch[], more = false) {
  const grep = new GrepState();
  grep.at = { repo: '/repo', rev: 'abc123', label: 'main' };
  grep.pattern = 'count';
  grep.result = { matches, more };
  grep.asked = { pattern: 'count', how: 'fixed', ignoreCase: true, paths: [] };
  const opened: GrepMatch[] = [];
  const view = render(Grep, { props: { grep, onOpen: (m: GrepMatch) => opened.push(m), onClose: () => {} } });
  return { grep, view, opened };
}

describe('the code search panel', () => {
  it('lists what it found under each file, with the match marked, and opens a line on a click', async () => {
    const { view, opened } = shown([
      { path: 'src/a.c', line: 3, column: 5, text: 'int count;' },
      { path: 'src/a.c', line: 9, column: 2, text: '\tCount++;' },
      { path: 'src/b.c', line: 1, column: 1, text: 'count' },
    ]);
    const files = [...view.container.querySelectorAll('.file .path')].map((p) => p.textContent);
    expect(files).toEqual(['src/a.c', 'src/b.c']);
    expect(view.container.querySelector('.status')?.textContent).toContain('3 lines in 2 files');
    const marks = [...view.container.querySelectorAll('.hit mark')].map((m) => m.textContent);
    expect(marks).toEqual(['count', 'Count', 'count']);

    await fireEvent.click(view.container.querySelectorAll('.hit')[1] as HTMLElement);
    expect(opened.map((m) => [m.path, m.line])).toEqual([['src/a.c', 9]]);
  });

  it('says when it stopped short, and when nothing matches', () => {
    const many = shown([{ path: 'a.c', line: 1, column: 1, text: 'count' }], true);
    expect(many.view.container.querySelector('.status')?.textContent).toContain('Narrow the search');
    many.view.unmount();
    const none = shown([]);
    expect(none.view.container.querySelector('.status')?.textContent).toContain('Nothing matches');
  });

  it('turns case and expressions on and off, and searches again', async () => {
    const { grep, view } = shown([]);
    const set = vi.spyOn(grep, 'set');
    await fireEvent.click(view.container.querySelector('button[title="Match case"]') as HTMLElement);
    await fireEvent.click(view.container.querySelector('button[title="Read it as a regular expression"]') as HTMLElement);
    expect(set.mock.calls).toEqual([[{ ignoreCase: false }], [{ how: 'regex' }]]);
  });
});
