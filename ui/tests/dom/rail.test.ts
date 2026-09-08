// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it, vi } from 'vitest';

import Rail from '../../src/app/Rail.svelte';

afterEach(cleanup);

/** The rail as a repository with something in every section shows it. */
function rail(over: Record<string, unknown> = {}) {
  const onOpen = vi.fn();
  return {
    onOpen,
    ...render(Rail, {
      props: {
        counts: { local: 12, remote: 2, stashes: 1, tags: 944, prs: 3, submodules: 1 },
        host: 'github',
        onOpen,
        ...over,
      },
    }),
  };
}

function labels(container: HTMLElement): (string | null)[] {
  return [...container.querySelectorAll('button')].map((b) => b.getAttribute('aria-label'));
}

describe('the left panel minimised', () => {
  it('lists the panel’s own sections, in the panel’s own order', () => {
    // It is a map of the panel, not a second menu: an icon in a different place from the
    // section it stands for is worse than no icon.
    expect(labels(rail().container)).toEqual([
      'Local', 'Remote', 'Stashes', 'Tags', 'Requests', 'Submodules',
    ]);
  });

  it('leaves out a section this repository does not have', () => {
    // The panel itself draws no submodule heading where there are no submodules, and a rail
    // that offered one would open the panel onto nothing.
    const { container } = rail({ counts: { local: 3, remote: 1, stashes: 0, tags: 0 } });
    expect(labels(container)).toEqual(['Local', 'Remote', 'Stashes', 'Tags']);
  });

  it('keeps a section the panel draws empty, because the panel draws it', () => {
    // The stash list is always there, saying "nothing stashed". The rail is a map of the
    // panel, so it says the same in its own tooltip rather than dropping the icon.
    const { container } = rail({ counts: { local: 1, remote: 0, stashes: 0, tags: 0 } });
    expect(labels(container)).toContain('Stashes');
    expect(
      container.querySelector('[aria-label="Stashes"]')?.getAttribute('title'),
    ).toBe('Stashes — 0 stashes');
  });

  it('carries the count in the tooltip, which is what the width costs it', () => {
    const { container } = rail();
    const tip = (label: string) =>
      container.querySelector(`[aria-label="${label}"]`)?.getAttribute('title');
    expect(tip('Tags')).toBe('Tags — 944 tags');
    expect(tip('Stashes'), 'one of a thing is not one things').toBe('Stashes — 1 stash');
    // Neither of these pluralises by adding an s, which is what an `n === 1 ? '' : 's'` did.
    expect(tip('Local')).toBe('Local — 12 branches');
  });

  it('asks the window to open the panel at the section that was clicked', () => {
    const { container, onOpen } = rail();
    void fireEvent.click(container.querySelector('[aria-label="Stashes"]') as HTMLElement);
    expect(onOpen.mock.calls).toEqual([['stashes']]);
  });

  it('marks the remote section with the host the remotes point at', () => {
    // The panel's own remote heading does this, and the rail stands for the panel.
    const { container } = rail({ host: 'gitlab' });
    const mark = container.querySelector('[aria-label="Remote"] svg');
    expect(mark, 'the host mark is drawn').not.toBeNull();
  });

  it('is narrow enough to be worth having, and paints its own ground', () => {
    // A surface carrying anything drawn has to paint an opaque background of its own or WebKit
    // stops antialiasing what is on it with subpixel precision.
    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText);
    const rule = rules.find((text) => /\.rail[^{]*\{/u.test(text) && /width:/u.test(text));
    expect(rule, 'the rail has a width of its own').toBeDefined();
    expect(rule).toMatch(/background:\s*var\(--bg-1\)/u);
    const width = Number.parseInt(/width:\s*(\d+)px/u.exec(rule ?? '')?.[1] ?? '999', 10);
    expect(width, 'a fraction of the panel it stands for').toBeLessThan(60);
  });
});
