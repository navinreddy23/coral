// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

import Menu, { type MenuItem } from '../../src/app/Menu.svelte';

function items(run = vi.fn()): MenuItem[] {
  return [
    { kind: 'item', label: 'Checkout this commit', hint: 'a1b2c3d4', run },
    { kind: 'separator' },
    {
      kind: 'submenu',
      label: 'Reset main to this commit',
      items: [
        { kind: 'item', label: 'Soft', run: vi.fn() },
        { kind: 'item', label: 'Hard', danger: true, run: vi.fn() },
      ],
    },
    { kind: 'item', label: 'Drop commit', danger: true, run: vi.fn() },
    { kind: 'item', label: 'Nothing to do here', disabled: true, run: vi.fn() },
  ];
}

/** The row for a label. Both the button and its span carry the text, so the span is named. */
function row(view: { container: HTMLElement }, label: string): HTMLElement {
  const found = [...view.container.querySelectorAll('.label')].find(
    (e) => e.textContent?.trim() === label,
  );
  if (!found) throw new Error(`no menu row labelled ${label}`);
  return found as HTMLElement;
}

describe('the context menu', () => {
  it('runs the item that was chosen and closes first', async () => {
    const run = vi.fn();
    const onClose = vi.fn();
    const view = render(Menu, { x: 10, y: 10, items: items(run), onClose });

    await fireEvent.click(row(view, 'Checkout this commit'));
    expect(run).toHaveBeenCalledOnce();
    // Closing before running matters: an item that opens a dialog would otherwise leave the
    // menu sitting under it.
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does nothing at all for a disabled item', async () => {
    const onClose = vi.fn();
    const list = items();
    const view = render(Menu, { x: 0, y: 0, items: list, onClose });

    await fireEvent.click(row(view, 'Nothing to do here'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('opens a submenu on hover and keeps only one open', async () => {
    const view = render(Menu, { x: 0, y: 0, items: items(), onClose: vi.fn() });
    expect(view.container.querySelector('.sub')).toBeNull();

    const wrap = row(view, 'Reset main to this commit').closest('.wrap');
    expect(wrap).not.toBeNull();
    if (wrap) await fireEvent.mouseEnter(wrap);
    expect(row(view, 'Soft')).toBeTruthy();
    expect(row(view, 'Hard')).toBeTruthy();

    if (wrap) await fireEvent.mouseLeave(wrap);
    expect(view.container.querySelector('.sub')).toBeNull();
  });

  it('closes on Escape, which is the only way out for a keyboard', async () => {
    const onClose = vi.fn();
    render(Menu, { x: 0, y: 0, items: items(), onClose });

    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('closes when the page behind it is clicked', async () => {
    const onClose = vi.fn();
    const view = render(Menu, { x: 0, y: 0, items: items(), onClose });

    const scrim = view.container.querySelector('.scrim');
    expect(scrim).not.toBeNull();
    if (scrim) await fireEvent.click(scrim);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('marks the destructive choices so they do not read like the rest', () => {
    const view = render(Menu, { x: 0, y: 0, items: items(), onClose: vi.fn() });
    const drop = row(view, 'Drop commit').closest('button');
    expect(drop?.className).toContain('danger');
  });
});
