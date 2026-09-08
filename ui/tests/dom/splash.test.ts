// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import Splash from '../../src/app/Splash.svelte';

describe('the loading screen', () => {
  it('names the repository and the step that is running', () => {
    const { container } = render(Splash, {
      props: { repo: 'linux', path: '/src/linux', stage: 'walking' },
    });
    expect(container.querySelector('h2')?.textContent).toBe('linux');
    expect(container.querySelector('.says')?.textContent).toBe('Walking the commit graph');
  });

  it('marks the steps before the current one done, and the current one running', () => {
    const { container } = render(Splash, {
      props: { repo: 'linux', path: '/src/linux', stage: 'ordering' },
    });
    const rows = [...container.querySelectorAll('.rail li')];
    expect(rows).toHaveLength(3);
    expect(rows[0]?.classList.contains('done')).toBe(true);
    expect(rows[1]?.classList.contains('done')).toBe(true);
    expect(rows[2]?.classList.contains('now')).toBe(true);
  });

  /** Every lane is normalised, or one dash animation cannot draw all three. */
  it('draws its graph with normalised path lengths', () => {
    const { container } = render(Splash, {
      props: { repo: 'linux', path: '/src/linux', stage: 'opening' },
    });
    const lanes = [...container.querySelectorAll('svg path')];
    expect(lanes.length).toBeGreaterThan(3);
    expect(lanes.every((p) => p.getAttribute('pathLength') === '1')).toBe(true);
    expect(container.querySelectorAll('svg circle')).toHaveLength(10);
  });
});
