// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import TabBar from '../../src/app/TabBar.svelte';
import { TabsState } from '../../src/state/tabs.svelte';

const GROUP = 7;

/** Two tabs in a group called "work", and one loose. */
function session() {
  return {
    tabs: [
      { id: 1, path: '/repos/alpha', group: GROUP, missing: false },
      { id: 2, path: '/repos/beta', group: GROUP, missing: false },
      { id: 3, path: '/repos/gamma', group: null, missing: false },
    ],
    groups: [{ id: GROUP, name: 'work', colour: 'lane1', collapsed: false }],
    active: 1,
  };
}

function bar() {
  const tabs = new TabsState();
  tabs.session = session();
  return { tabs, ...render(TabBar, { props: { tabs, onOpen: () => {} } }) };
}

const transfer = () => ({ setData: () => {}, effectAllowed: '', dropEffect: '' });

function chipFor(container: HTMLElement, name: string): HTMLElement {
  const found = [...container.querySelectorAll('.tab')].find((t) =>
    t.querySelector('.pick')?.textContent?.includes(name),
  );
  expect(found, `a tab for ${name}`).toBeDefined();
  return found as HTMLElement;
}

/** The arguments of the last tab_move, which is what a drop is. */
function lastMove(): Record<string, unknown> | undefined {
  const call = invoke.mock.calls.filter(([cmd]) => cmd === 'tab_move').at(-1);
  return call?.[1] as Record<string, unknown> | undefined;
}

describe('dragging a tab', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async () => session());
  });

  it('joins a group when dropped on its band', async () => {
    // The thing that was not possible before: a group could be created and a tab taken out of
    // one, but nothing put a tab into a group that already existed.
    const { container } = bar();
    const gamma = chipFor(container, 'gamma');
    const band = container.querySelector('.band') as HTMLElement;

    await fireEvent.dragStart(gamma, { dataTransfer: transfer() });
    await fireEvent.dragOver(band, { dataTransfer: transfer() });
    await fireEvent.drop(band, { dataTransfer: transfer() });

    await waitFor(() => {
      expect(lastMove()).toEqual({ id: 3, group: GROUP, before: null });
    });
  });

  it('lands in front of the tab it is dropped on, in that tab’s group', async () => {
    const { container } = bar();
    const gamma = chipFor(container, 'gamma');
    const beta = chipFor(container, 'beta');

    await fireEvent.dragStart(gamma, { dataTransfer: transfer() });
    await fireEvent.dragOver(beta, { dataTransfer: transfer() });
    await fireEvent.drop(beta, { dataTransfer: transfer() });

    await waitFor(() => {
      expect(lastMove()).toEqual({ id: 3, group: GROUP, before: 2 });
    });
  });

  it('leaves every group when dropped on the bar itself', async () => {
    const { container } = bar();
    const alpha = chipFor(container, 'alpha');
    const nav = container.querySelector('nav.bar') as HTMLElement;

    await fireEvent.dragStart(alpha, { dataTransfer: transfer() });
    await fireEvent.dragOver(nav, { dataTransfer: transfer() });
    await fireEvent.drop(nav, { dataTransfer: transfer() });

    await waitFor(() => {
      expect(lastMove()).toEqual({ id: 1, group: null, before: null });
    });
  });

  it('does nothing when a tab is dropped on itself', async () => {
    const { container } = bar();
    const alpha = chipFor(container, 'alpha');

    await fireEvent.dragStart(alpha, { dataTransfer: transfer() });
    await fireEvent.dragOver(alpha, { dataTransfer: transfer() });
    await fireEvent.drop(alpha, { dataTransfer: transfer() });

    expect(lastMove()).toBeUndefined();
  });

  it('shows where the tab would land while it is being dragged', async () => {
    const { container } = bar();
    const gamma = chipFor(container, 'gamma');
    const beta = chipFor(container, 'beta');

    await fireEvent.dragStart(gamma, { dataTransfer: transfer() });
    expect(gamma.className).toContain('dragging');

    await fireEvent.dragOver(beta, { dataTransfer: transfer() });
    // An insertion line on the tab it would go in front of.
    expect(beta.className).toContain('before');

    await fireEvent.dragEnd(gamma);
    expect(container.querySelector('.tab.before')).toBeNull();
    expect(container.querySelector('.tab.dragging')).toBeNull();
  });

  it('marks the band a drop would join', async () => {
    const { container } = bar();
    const gamma = chipFor(container, 'gamma');
    const band = container.querySelector('.band') as HTMLElement;

    await fireEvent.dragStart(gamma, { dataTransfer: transfer() });
    await fireEvent.dragOver(band, { dataTransfer: transfer() });
    expect(band.className).toContain('target');

    await fireEvent.dragLeave(band);
    expect(band.className).not.toContain('target');
  });

  it('ignores a drag that did not start on a tab', async () => {
    // A file dragged in from elsewhere must not rearrange the bar.
    const { container } = bar();
    const band = container.querySelector('.band') as HTMLElement;
    await fireEvent.dragOver(band, { dataTransfer: transfer() });
    await fireEvent.drop(band, { dataTransfer: transfer() });
    expect(lastMove()).toBeUndefined();
  });

  it('a drop on a tab does not also count as a drop on the bar', async () => {
    // The bar is the tab's ancestor, so without stopping the event the tab would be moved
    // twice: once into the group and once straight out of it.
    const { container } = bar();
    const gamma = chipFor(container, 'gamma');
    const beta = chipFor(container, 'beta');

    await fireEvent.dragStart(gamma, { dataTransfer: transfer() });
    await fireEvent.drop(beta, { dataTransfer: transfer() });

    await waitFor(() => {
      expect(invoke.mock.calls.filter(([cmd]) => cmd === 'tab_move')).toHaveLength(1);
    });
  });
});
