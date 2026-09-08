// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import Shortcuts from '../../src/app/Shortcuts.svelte';
import { BINDINGS } from '../../src/state/shortcuts';

afterEach(cleanup);

const every = () => new Set(BINDINGS.map((b) => b.id));

describe('the keyboard sheet', () => {
  it('lists every binding, with its keys', () => {
    const { container } = render(Shortcuts, { props: { live: every(), onClose: vi.fn() } });
    const rows = [...container.querySelectorAll('.row')];
    expect(rows).toHaveLength(BINDINGS.length);
    for (const row of rows) {
      expect(row.querySelector('kbd')?.textContent?.trim().length ?? 0, row.textContent ?? '')
        .toBeGreaterThan(0);
    }
  });

  it('explains the dimmed rows only while there are some', () => {
    // The legend is for a state: with everything wired it is a line that sends the reader
    // looking for something that is not there.
    const all = render(Shortcuts, { props: { live: every(), onClose: vi.fn() } });
    expect(all.container.querySelector('.note')).toBeNull();
    cleanup();

    const some = new Set([...every()].slice(1));
    const { container } = render(Shortcuts, { props: { live: some, onClose: vi.fn() } });
    expect(container.querySelector('.note')?.textContent).toContain('not wired up');
    expect(container.querySelectorAll('.row.pending')).toHaveLength(1);
  });
});
