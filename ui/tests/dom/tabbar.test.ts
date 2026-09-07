// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import TabBar from '../../src/app/TabBar.svelte';
import { TabsState } from '../../src/state/tabs.svelte';

const GROUP = 7;

/** Two tabs in a group called "work", and one loose. */
function session() {
  return {
    tabs: [
      { id: 1, path: '/repos/alpha', submodule: null, group: GROUP, missing: false },
      { id: 2, path: '/repos/beta', submodule: null, group: GROUP, missing: false },
      { id: 3, path: '/repos/gamma', submodule: null, group: null, missing: false },
    ],
    groups: [{ id: GROUP, name: 'work', colour: 'lane1', collapsed: false }],
    active: 1,
  };
}

function bar() {
  const tabs = new TabsState();
  tabs.session = session();
  const onAsk = vi.fn(async () => 'named');
  const onPick = vi.fn();
  return {
    tabs,
    onAsk,
    onPick,
    ...render(TabBar, { props: { tabs, onOpen: () => {}, onAsk, onPick } }),
  };
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

describe('the tab search', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async () => session());
  });

  async function open(container: HTMLElement) {
    const button = container.querySelector('.find') as HTMLElement;
    expect(button, 'the search button').toBeTruthy();
    await fireEvent.click(button);
  }

  it('lists every open tab, whichever group it is in', async () => {
    // The bar scrolls once there are more tabs than fit, and a scrolled bar is no way to find
    // one repository among twenty.
    const { container } = bar();
    await open(container);

    const names = [...container.querySelectorAll('.finder .name')].map((e) => e.textContent);
    expect(names).toEqual(['alpha', 'beta', 'gamma']);
  });

  it('says which group each tab belongs to', async () => {
    const { container } = bar();
    await open(container);

    const tags = [...container.querySelectorAll('.finder .tag')].map((e) => e.textContent);
    expect(tags).toEqual(['work', 'work']);
  });

  it('filters on the whole path, not only the name', async () => {
    // Two checkouts of the same repository have the same name and differ only in where they
    // are, so matching the name alone cannot tell them apart.
    const { container } = bar();
    await open(container);

    const field = container.querySelector('.finder input') as HTMLInputElement;
    await fireEvent.input(field, { target: { value: '/repos/be' } });

    const names = [...container.querySelectorAll('.finder .name')].map((e) => e.textContent);
    expect(names).toEqual(['beta']);
  });

  it('activates the first match on Enter', async () => {
    const { container } = bar();
    await open(container);

    const field = container.querySelector('.finder input') as HTMLInputElement;
    await fireEvent.input(field, { target: { value: 'gamma' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => {
      const call = invoke.mock.calls.filter(([cmd]) => cmd === 'tab_activate').at(-1);
      expect(call?.[1]).toEqual({ id: 3 });
    });
  });

  it('says so rather than showing an empty list when nothing matches', async () => {
    const { container } = bar();
    await open(container);

    const field = container.querySelector('.finder input') as HTMLInputElement;
    await fireEvent.input(field, { target: { value: 'nothing like this' } });

    expect(container.querySelector('.finder .none')).toBeTruthy();
    expect(container.querySelectorAll('.finder .name')).toHaveLength(0);
  });

  it('closes on Escape', async () => {
    const { container } = bar();
    await open(container);
    expect(container.querySelector('.finder')).toBeTruthy();

    const field = container.querySelector('.finder input') as HTMLInputElement;
    await fireEvent.keyDown(field, { key: 'Escape' });
    expect(container.querySelector('.finder')).toBeNull();
  });

  it('closes a tab from the list without leaving the search', async () => {
    const { container } = bar();
    await open(container);

    const drop = container.querySelectorAll('.finder .drop')[2] as HTMLElement;
    await fireEvent.click(drop);

    await waitFor(() => {
      const call = invoke.mock.calls.filter(([cmd]) => cmd === 'tab_close').at(-1);
      expect(call?.[1]).toEqual({ id: 3 });
    });
    expect(container.querySelector('.finder')).toBeTruthy();
  });
});

describe('the tab and group menus', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async () => session());
  });

  it('offers to put a loose tab into a group that already exists', async () => {
    const { container } = bar();
    await fireEvent.contextMenu(chipFor(container, 'gamma'));

    const labels = [...container.querySelectorAll('.menu .label')].map((e) => e.textContent?.trim());
    expect(labels).toContain('New tab group…');
    expect(labels).toContain('Add to group');
  });

  it('offers to take a grouped tab out again, and does not offer that to a loose one', async () => {
    const { container } = bar();
    await fireEvent.contextMenu(chipFor(container, 'alpha'));
    let labels = [...container.querySelectorAll('.menu .label')].map((e) => e.textContent?.trim());
    expect(labels).toContain('Remove from group');

    await fireEvent.keyDown(window, { key: 'Escape' });
    await fireEvent.contextMenu(chipFor(container, 'gamma'));
    labels = [...container.querySelectorAll('.menu .label')].map((e) => e.textContent?.trim());
    expect(labels).not.toContain('Remove from group');
  });

  it('offers to rename, recolour, dissolve and close a group', async () => {
    const { container } = bar();
    const name = container.querySelector('.group') as HTMLElement;
    await fireEvent.contextMenu(name);

    const labels = [...container.querySelectorAll('.menu .label')].map((e) => e.textContent?.trim());
    expect(labels).toContain('Rename group…');
    expect(labels).toContain('Colour');
    expect(labels).toContain('Ungroup, keeping the tabs');
    expect(labels).toContain('Close every tab in the group');
  });

  it('dissolving a group keeps the tabs, which is not the same as closing them', async () => {
    const { container } = bar();
    await fireEvent.contextMenu(container.querySelector('.group') as HTMLElement);

    const item = [...container.querySelectorAll('.menu .label')].find(
      (e) => e.textContent?.trim() === 'Ungroup, keeping the tabs',
    ) as HTMLElement;
    await fireEvent.click(item);

    await waitFor(() => {
      expect(invoke.mock.calls.some(([cmd]) => cmd === 'group_dissolve')).toBe(true);
    });
    expect(invoke.mock.calls.some(([cmd]) => cmd === 'group_close')).toBe(false);
  });
});

describe('the picture on a tab', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(session());
  });

  it('draws one on every tab, and a colour that is the same for the same repository', () => {
    const { container } = bar();
    const marks = [...container.querySelectorAll('.tab .icon')];
    expect(marks).toHaveLength(3);
    expect(marks.every((m) => m.querySelector('svg') !== null)).toBe(true);

    const hue = (name: string) =>
      (chipFor(container, name).querySelector('.icon') as HTMLElement).style.getPropertyValue(
        '--hue',
      );
    expect(hue('alpha')).toMatch(/^var\(--lane-[1-8]\)$/u);
    // Two checkouts differing only in their last segment must not collide, or the colour says
    // nothing. Not a property of the hash in general, but it has to hold for what is on screen.
    expect(new Set([hue('alpha'), hue('beta'), hue('gamma')]).size).toBeGreaterThan(1);
  });

  it('opens a grid from the tab menu and sends the one that is clicked', async () => {
    const { container } = bar();
    await fireEvent.contextMenu(chipFor(container, 'gamma'));
    const open = [...container.querySelectorAll('.menu .label')].find(
      (e) => e.textContent?.trim() === 'Change icon…',
    ) as HTMLElement;
    await fireEvent.click(open);

    const cells = [...container.querySelectorAll('.picker .cell')];
    expect(cells).toHaveLength(12);
    const rocket = cells.find((c) => c.getAttribute('aria-label') === 'Release') as HTMLElement;
    await fireEvent.click(rocket);

    await waitFor(() => {
      const call = invoke.mock.calls.filter(([cmd]) => cmd === 'tab_icon').at(-1);
      expect(call?.[1]).toEqual({ id: 3, icon: 'rocket' });
    });
  });

  it('puts a tab back to the default with null rather than with a name', async () => {
    const { container } = bar();
    await fireEvent.contextMenu(chipFor(container, 'gamma'));
    await fireEvent.click(
      [...container.querySelectorAll('.menu .label')].find(
        (e) => e.textContent?.trim() === 'Change icon…',
      ) as HTMLElement,
    );
    await fireEvent.click(container.querySelector('.picker .reset') as HTMLElement);

    await waitFor(() => {
      const call = invoke.mock.calls.filter(([cmd]) => cmd === 'tab_icon').at(-1);
      expect(call?.[1]).toEqual({ id: 3, icon: null });
    });
  });
});

describe('a collapsed group', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(session());
  });

  /**
   * The count lives on the chip, and its colours have to come from the chip's own rule.
   * A second `.tally` elsewhere in the sheet once won the cascade and painted it the strip's
   * grey while it still inherited the chip's white text, which left the number invisible.
   */
  it('shows how many tabs it is hiding, on the chip itself', () => {
    const tabs = new TabsState();
    const s = session();
    s.groups[0].collapsed = true;
    tabs.session = s;
    const { container } = render(TabBar, { props: { tabs, onOpen: () => {}, onAsk: vi.fn() } });

    const tally = container.querySelector('.group .tally');
    expect(tally?.textContent?.trim()).toBe('2');
    expect(container.querySelectorAll('.band .tab')).toHaveLength(0);
  });
});

describe('the start page and the tabs', () => {
  /**
   * The start page is not a tab. Picking a real one used to leave it up: the repository loaded
   * behind a page nobody had asked to keep, and the only way out was its own close button.
   */
  it('says a tab was picked, so the page over it can go', async () => {
    const { container, onPick } = bar();
    await fireEvent.click(chipFor(container, 'gamma').querySelector('.pick') as HTMLElement);
    expect(onPick).toHaveBeenCalledTimes(1);
  });

  it('says so for the tab that is already current, which is the one over the page', async () => {
    const { container, onPick } = bar();
    await fireEvent.click(chipFor(container, 'alpha').querySelector('.pick') as HTMLElement);
    expect(onPick).toHaveBeenCalledTimes(1);
  });
});

describe('a strip with more tabs than fit', () => {
  /**
   * The new-tab button belongs after the last tab, where a browser puts it and where the hand
   * goes looking for it. It was moved out to the far end once because, inside the strip that
   * scrolls, it went off the end with the last tab: past a dozen repositories there was no way
   * to open another. It is back in the strip and stays reachable by sticking to the trailing
   * edge instead, which is checked below.
   */
  it('puts the new-tab button after the last tab', () => {
    const { container } = bar();
    const nav = container.querySelector('nav.bar') as HTMLElement;
    const add = nav.querySelector('.add');
    expect(add, 'the plus is in the strip with the tabs').not.toBeNull();

    const tabs = [...nav.querySelectorAll('.tab')];
    const last = tabs[tabs.length - 1];
    expect(
      last.compareDocumentPosition(add as Node) & Node.DOCUMENT_POSITION_FOLLOWING,
      'and after the last of them',
    ).toBeTruthy();
  });

  it('sticks the new-tab button to the trailing edge so it cannot scroll away', () => {
    const { container } = bar();
    const add = container.querySelector('nav.bar .add') as HTMLElement;
    // The one property that keeps it reachable when the strip overflows. Without it the
    // button is adjacent to the last tab and unreachable, which is the arrangement this
    // replaced.
    expect(getComputedStyle(add).position).toBe('sticky');
  });

  it('keeps the tab search out of the scrolling strip', () => {
    // Unlike the plus, the search has no natural place among the tabs: it exists for the case
    // where there are too many of them to look through, so it is pinned outside them.
    const { container } = bar();
    expect(container.querySelector('nav.bar .find'), 'not in the scroll').toBeNull();
    expect(container.querySelector('.tail .find'), 'in the tail').not.toBeNull();
  });
});
