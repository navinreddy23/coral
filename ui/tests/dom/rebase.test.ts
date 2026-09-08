// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import RebasePicker from '../../src/app/RebasePicker.svelte';
import { RebaseState } from '../../src/state/rebase.svelte';
import type { TodoItem } from '../../src/ipc/types';

function item(oid: string, summary: string): TodoItem {
  return { step: 'pick', oid: oid.padEnd(40, '0'), summary, message: null };
}

function picker(items: TodoItem[]) {
  const rebase = new RebaseState();
  rebase.onto = 'main';
  rebase.items = items;
  return { rebase, ...render(RebasePicker, { props: { rebase, onDone: () => {} } }) };
}

const three = () => [item('aaa', 'commit a'), item('bbb', 'commit b'), item('ccc', 'commit c')];

describe('the interactive rebase picker', () => {
  it('lists the range oldest first, one row per commit', () => {
    const { container } = picker(three());
    const rows = [...container.querySelectorAll('li')];
    expect(rows).toHaveLength(3);
    expect(rows[0]?.textContent).toContain('commit a');
    expect(rows[2]?.textContent).toContain('commit c');
  });

  it('counts what will survive as commits are dropped', () => {
    const { rebase, container } = picker(three());
    expect(container.textContent).toContain('3 of 3 commits kept');
    rebase.setStep(1, 'drop');
    expect(rebase.remaining).toBe(2);
  });

  it('leaves a dropped commit in place rather than removing the row', () => {
    // Its position still matters: it says where the ones around it sit.
    const { rebase, container } = picker(three());
    rebase.setStep(1, 'drop');
    expect(container.querySelectorAll('li')).toHaveLength(3);
  });

  it('refuses a list whose first commit has nothing to fold into', async () => {
    const { rebase, container } = picker(three());
    rebase.setStep(0, 'squash');
    expect(rebase.invalid).toBe(true);
    await Promise.resolve();
    const start = [...container.querySelectorAll('footer button')].find(
      (b) => b.textContent?.includes('Start'),
    ) as HTMLButtonElement;
    expect(start.disabled).toBe(true);
    expect(container.querySelector('.warn')?.textContent).toContain('nothing above it');
  });

  it('offers a message field for a reword, seeded with the message it has', async () => {
    const { rebase, container } = picker(three());
    rebase.setStep(1, 'reword');
    await Promise.resolve();
    const field = container.querySelector('input.message') as HTMLInputElement;
    expect(field).not.toBeNull();
    // Usually an edit of the message rather than a replacement, so it starts from it.
    expect(field.value).toBe('commit b');
  });

  it('will not start a reword with nothing to say', async () => {
    const { rebase, container } = picker(three());
    rebase.setStep(1, 'reword');
    rebase.setMessage(1, '   ');
    expect(rebase.invalid).toBe(true);
    await Promise.resolve();
    const start = [...container.querySelectorAll('footer button')].find((b) =>
      b.textContent?.includes('Start'),
    ) as HTMLButtonElement;
    expect(start.disabled).toBe(true);
  });

  it('forgets the message when the step stops being a reword', () => {
    const rebase = new RebaseState();
    rebase.items = three();
    rebase.setStep(0, 'reword');
    rebase.setMessage(0, 'a new message');
    rebase.setStep(0, 'pick');
    // A message left on a picked commit would be sent and quietly ignored.
    expect(rebase.items[0]?.message).toBeNull();
  });
});

describe('reordering', () => {
  it('moves one commit and keeps the rest in order', () => {
    const rebase = new RebaseState();
    rebase.items = three();
    rebase.move(2, 0);
    expect(rebase.items.map((i) => i.summary)).toEqual(['commit c', 'commit a', 'commit b']);
  });

  it('ignores a move that goes nowhere or off the end', () => {
    const rebase = new RebaseState();
    rebase.items = three();
    for (const [from, to] of [[1, 1], [-1, 0], [0, 9], [9, 0]] as const) {
      rebase.move(from, to);
      expect(rebase.items.map((i) => i.summary)).toEqual(['commit a', 'commit b', 'commit c']);
    }
  });

  it('reorders by dragging one row onto another', async () => {
    const { rebase, container } = picker(three());
    const rows = [...container.querySelectorAll('li')];
    await fireEvent.dragStart(rows[2] as HTMLElement);
    await fireEvent.drop(rows[0] as HTMLElement);
    expect(rebase.items.map((i) => i.summary)).toEqual(['commit c', 'commit a', 'commit b']);
  });
});

describe('what the picker calls the target', () => {
  it('cuts an object id down, and leaves a name alone', () => {
    const long = 'd38081e90dcfd1183977998a03aed3fe8e324949';
    const { container } = picker([item('aaa', 'one')]);
    expect(container.querySelector('header')?.textContent).toContain('main');

    const second = new RebaseState();
    second.onto = `${long}~1`;
    second.items = [item('aaa', 'one')];
    const { container: box } = render(RebasePicker, {
      props: { rebase: second, onDone: () => {} },
    });
    const said = box.querySelector('header')?.textContent ?? '';
    expect(said).toContain('d38081e9~1');
    expect(said).not.toContain(long);
  });
});
