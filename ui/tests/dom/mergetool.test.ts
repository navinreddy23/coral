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

import MergeTool from '../../src/app/MergeTool.svelte';
import { MergeState } from '../../src/state/merge.svelte';
import type { ConflictedFile, Operation } from '../../src/ipc/types';

function operation(swapped = false): Operation {
  return {
    state: swapped ? 'rebase' : 'merge',
    labels: swapped
      ? { ours: 'main', theirs: 'feature', swapped: true }
      : { ours: 'main', theirs: 'side', swapped: false },
    progress: null,
    headName: null,
    stoppedAt: null,
    interactive: false,
  };
}

function conflicted(over: Partial<ConflictedFile> = {}): ConflictedFile {
  return { path: 'f.txt', kind: 'both_modified', binary: false, deleteModify: false, ...over };
}

function state(files: ConflictedFile[], swapped = false) {
  const merge = new MergeState();
  merge.operation = operation(swapped);
  merge.files = files;
  return merge;
}

const noop = () => {};

describe('the merge tool', () => {
  it('will not continue while a file is still conflicted', () => {
    const { container } = render(MergeTool, { props: { merge: state([conflicted()]), onDone: noop } });
    const buttons = [...container.querySelectorAll('header button')];
    const cont = buttons.find((b) => b.textContent?.trim() === 'Continue');
    expect(cont).toBeDefined();
    expect((cont as HTMLButtonElement).disabled).toBe(true);
  });

  it('lets the operation finish once nothing is left', () => {
    const { container } = render(MergeTool, { props: { merge: state([]), onDone: noop } });
    const cont = [...container.querySelectorAll('header button')].find(
      (b) => b.textContent?.trim() === 'Continue',
    );
    expect((cont as HTMLButtonElement).disabled).toBe(false);
    expect(container.textContent).toContain('Every file is resolved');
  });

  it('names the sides after the branches, not "ours" and "theirs"', () => {
    const { container } = render(MergeTool, { props: { merge: state([conflicted()]), onDone: noop } });
    const wholesale = container.querySelector('.wholesale')?.textContent ?? '';
    expect(wholesale).toContain('main');
    expect(wholesale).toContain('side');
  });

  it('warns that a rebase reverses the sides', () => {
    const { container } = render(MergeTool, {
      props: { merge: state([conflicted()], true), onDone: noop },
    });
    expect(container.querySelector('.warn')?.textContent).toContain('reversed');
  });

  it('offers only whole-file choices for a binary conflict', async () => {
    const merge = state([conflicted({ binary: true })]);
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const file = container.querySelector('button.file') as HTMLButtonElement;
    expect(file.textContent).toContain('whole file');

    // Selectable all the same: there is still a choice to make, and disabling it left the
    // pane telling people to pick regions in a file that has none.
    expect(file.disabled).toBe(false);
    await fireEvent.click(file);
    expect(container.querySelector('.whole')?.textContent).toContain('binary');
    expect(container.querySelector('.conflict')).toBeNull();
  });

  it('explains a file deleted on this side and changed by the commit, and offers the two ways out', async () => {
    // The reported case: cherry-picking a commit that changes a file this branch does not
    // have. There is one version of it, so a region picker has nothing to pick between.
    const merge = state([conflicted({ kind: 'deleted_by_us', deleteModify: true })]);
    merge.operation = {
      state: 'cherry_pick',
      labels: { ours: 'dummyx', theirs: '3755eae (test conflicts)', swapped: false },
      progress: null,
      headName: null,
      stoppedAt: null,
      interactive: false,
    };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });

    expect(container.querySelector('header')?.textContent).toContain('cherry-pick in progress');
    // The side that does not exist is not offered; taking it could only ever fail.
    const wholesale = container.querySelector('.wholesale')?.textContent ?? '';
    expect(wholesale).not.toContain('dummyx');
    expect(wholesale).toContain('3755eae');

    await fireEvent.click(container.querySelector('button.file') as HTMLButtonElement);
    const said = container.querySelector('.whole')?.textContent ?? '';
    expect(said).toContain('is not on dummyx');
    const choices = [...container.querySelectorAll('.choices button')].map((b) =>
      b.textContent?.trim(),
    );
    expect(choices).toEqual(['Take the version from 3755eae (test conflicts)', 'Leave it deleted']);
  });

  it('offers a delete for a file removed on one side', () => {
    const merge = state([conflicted({ deleteModify: true })]);
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    expect(container.querySelector('.wholesale')?.textContent).toContain('delete');
  });

  it('marks a region undecided until a side is picked, and blocks resolving', () => {
    const merge = state([conflicted()]);
    merge.active = 'f.txt';
    merge.blocks = {
      blocks: [
        { kind: 'common', lines: ['one'] },
        { kind: 'conflict', base: ['two'], ours: ['MAIN'], theirs: ['SIDE'] },
      ],
    };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    expect(container.querySelector('.conflict.undecided')).not.toBeNull();
    const resolve = [...container.querySelectorAll('.bar button')].find(
      (b) => b.textContent?.trim() === 'Mark resolved',
    ) as HTMLButtonElement;
    expect(resolve.disabled).toBe(true);
  });

  it('offers the base only where there is one', () => {
    const merge = state([conflicted()]);
    merge.active = 'f.txt';
    merge.blocks = {
      blocks: [{ kind: 'conflict', base: [], ours: ['MAIN'], theirs: ['SIDE'] }],
    };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    // An add/add conflict has no base; a third empty column would be noise.
    expect(container.querySelector('.side.base')).toBeNull();
    expect(container.querySelectorAll('.side')).toHaveLength(2);
  });

  it('says which commit failed to apply when a step stops on the next one', () => {
    // A rebase stops once per conflicting commit. Continuing usually lands on the next one,
    // and without this the window looked identical to having finished: the file list refills
    // and nothing says why.
    const merge = state([conflicted()], true);
    merge.stopped = 'error: could not apply 91e605d... local: dummy1 file';
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    expect(container.textContent).toContain('could not apply 91e605d');
  });

  it('says nothing of the kind before a step has stopped', () => {
    const { container } = render(MergeTool, { props: { merge: state([conflicted()]), onDone: noop } });
    expect(container.querySelector('.stopped')).toBeNull();
  });
});

/**
 * Picking region by region, which is how a conflict is actually settled.
 *
 * The sides are toggles, not one choice of three: a great many conflicts are resolved by
 * keeping both lines, and the order they are taken in is part of the answer.
 */
describe('picking sides in the merge tool', () => {
  /** A state with one file open on a single conflicting region. */
  function opened(swapped = false) {
    const merge = state([conflicted()], swapped);
    merge.active = 'f.txt';
    merge.blocks = {
      blocks: [
        { kind: 'common', lines: ['one'] },
        { kind: 'conflict', base: ['two'], ours: ['MAIN'], theirs: ['SIDE'] },
      ],
    };
    return merge;
  }

  function sideNamed(container: HTMLElement, which: string): HTMLButtonElement {
    const found = container.querySelector(`.side.${which}`);
    if (!found) throw new Error(`no ${which} side`);
    return found as HTMLButtonElement;
  }

  it('keeps both sides when both are clicked, in the order they were clicked', async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(sideNamed(container, 'theirs'));
    await fireEvent.click(sideNamed(container, 'ours'));

    expect(merge.choices[0]).toEqual(['theirs', 'ours']);
    expect(merge.output).toBe('one\nSIDE\nMAIN\n');
  });

  it('numbers the taken sides once more than one is taken', async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(sideNamed(container, 'ours'));
    // One side taken needs no number: there is no order to show.
    expect(container.querySelector('.order')).toBeNull();
    await fireEvent.click(sideNamed(container, 'theirs'));
    expect([...container.querySelectorAll('.order')].map((e) => e.textContent?.trim()))
      .toEqual(['1', '2']);
  });

  it('takes a side back out when it is clicked again, and says the region takes nothing', async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(sideNamed(container, 'ours'));
    await fireEvent.click(sideNamed(container, 'ours'));

    expect(merge.choices[0]).toEqual([]);
    expect(container.querySelector('.neither')).not.toBeNull();
    // Decided, though it keeps nothing, so the file can be resolved.
    const resolve = [...container.querySelectorAll('.bar button')].find(
      (b) => b.textContent?.trim() === 'Mark resolved',
    ) as HTMLButtonElement;
    expect(resolve.disabled).toBe(false);
  });

  it('shows the result that will be written, and follows the picks', async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(sideNamed(container, 'theirs'));
    expect(container.querySelector('pre.output')?.textContent).toBe('one\nSIDE\n');
  });

  it('lets the result be typed over, and the picks be gone back to', async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(sideNamed(container, 'ours'));

    const edit = [...container.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Edit it by hand',
    ) as HTMLButtonElement;
    await fireEvent.click(edit);

    const box = container.querySelector('textarea.output') as HTMLTextAreaElement;
    expect(box.value).toBe('one\nMAIN\n');
    await fireEvent.input(box, { target: { value: 'one\nsomething else\n' } });
    expect(merge.output).toBe('one\nsomething else\n');
    // The picks no longer decide the file, so they are not live either.
    expect(sideNamed(container, 'ours').disabled).toBe(true);

    const back = [...container.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Back to picking sides',
    ) as HTMLButtonElement;
    await fireEvent.click(back);
    expect(merge.output).toBe('one\nMAIN\n');
  });

  it('offers to skip a commit during a rebase, and not during a merge', () => {
    const rebase = render(MergeTool, { props: { merge: opened(true), onDone: noop } });
    const labels = [...rebase.container.querySelectorAll('header button')].map((b) =>
      b.textContent?.trim(),
    );
    expect(labels).toContain('Skip commit');

    // `git merge --skip` does not exist: there is one commit being made, and skipping it is
    // aborting.
    const merge = render(MergeTool, { props: { merge: opened(false), onDone: noop } });
    const during = [...merge.container.querySelectorAll('header button')].map((b) =>
      b.textContent?.trim(),
    );
    expect(during).not.toContain('Skip commit');
  });
});
