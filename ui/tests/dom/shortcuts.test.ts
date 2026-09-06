// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

import Shortcuts from '../../src/app/Shortcuts.svelte';

describe('the keyboard shortcuts sheet', () => {
  /**
   * The handler used to hang on the scrim, which needs focus to receive a key. Nothing gave it
   * any, so the sheet listing every shortcut was the one panel that ignored the one key
   * everybody presses to close a panel.
   */
  it('closes on Escape, wherever the key lands', async () => {
    const onClose = vi.fn();
    render(Shortcuts, { props: { live: new Set<string>(), onClose } });

    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('closes on a click outside the sheet', async () => {
    const onClose = vi.fn();
    const { container } = render(Shortcuts, { props: { live: new Set<string>(), onClose } });

    await fireEvent.click(container.querySelector('.scrim') as HTMLElement);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
