// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
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

  it('offers only whole-file choices for a binary conflict', () => {
    const merge = state([conflicted({ binary: true })]);
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const file = container.querySelector('button.file') as HTMLButtonElement;
    // There are no blocks to pick between, so opening it would show an empty pane.
    expect(file.disabled).toBe(true);
    expect(file.textContent).toContain('whole file');
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
