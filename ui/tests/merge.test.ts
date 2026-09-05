import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../src/ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const { MergeState, render } = await import('../src/state/merge.svelte');
const commands = await import('../src/ipc/commands');
import type { Block } from '../src/ipc/types';

const common = (...lines: string[]): Block => ({ kind: 'common', lines });
const conflict = (ours: string[], theirs: string[], base: string[] = []): Block => ({
  kind: 'conflict',
  base,
  ours,
  theirs,
});

describe('rendering a resolved file', () => {
  it('keeps the agreeing regions verbatim', () => {
    expect(render([common('a', 'b')], {})).toBe('a\nb\n');
  });

  it('takes the chosen side of each region independently', () => {
    const blocks = [
      common('top'),
      conflict(['mine'], ['yours']),
      common('middle'),
      conflict(['mine2'], ['yours2']),
      common('end'),
    ];
    const out = render(blocks, { 0: 'theirs', 1: 'ours' });
    expect(out).toBe('top\nyours\nmiddle\nmine2\nend\n');
  });

  it('can take the base, which is how a region is reverted', () => {
    const blocks = [conflict(['mine'], ['yours'], ['original'])];
    expect(render(blocks, { 0: 'base' })).toBe('original\n');
  });

  it('keeps our side where no decision was made', () => {
    // That is what git already staged, so applying early is never worse than not opening it.
    const blocks = [conflict(['mine'], ['yours'])];
    expect(render(blocks, {})).toBe('mine\n');
  });

  it('handles a side that contributes no lines', () => {
    // One side deleted the region entirely; taking it must not leave a blank line behind.
    const blocks = [common('a'), conflict([], ['yours']), common('b')];
    expect(render(blocks, { 0: 'ours' })).toBe('a\nb\n');
  });

  it('produces nothing for a file whose every line was dropped', () => {
    expect(render([conflict([], ['yours'])], { 0: 'ours' })).toBe('');
  });

  it('numbers decisions by conflict, not by block', () => {
    // The second conflict is index 1 even though it is the fourth block.
    const blocks = [common('a'), conflict(['x'], ['y']), common('b'), conflict(['p'], ['q'])];
    expect(render(blocks, { 1: 'theirs' })).toBe('a\nx\nb\nq\n');
  });
});

describe('stepping an operation on', () => {
  /** A state already loaded against a repository, with the reads stubbed out. */
  function loaded() {
    const merge = new MergeState();
    vi.spyOn(commands, 'repoOperation').mockResolvedValue({
      state: 'rebase',
      labels: { ours: 'main', theirs: 'topic', swapped: true },
      progress: { current: 2, total: 2 },
      headName: 'topic',
      stoppedAt: null,
      interactive: false,
    });
    vi.spyOn(commands, 'repoConflicts').mockResolvedValue([
      { path: 'dummy.txt', kind: 'both_modified', binary: false, deleteModify: false },
    ]);
    return merge;
  }

  it('keeps git’s reason when the rebase stops on the next commit', async () => {
    const merge = loaded();
    vi.spyOn(commands, 'operationStep').mockResolvedValue({
      completed: false,
      state: 'rebase',
      conflicts: ['dummy.txt'],
      message: 'error: could not apply 91e605d... local: dummy1 file',
    });

    expect(await merge.step('continue')).toBe(false);
    expect(merge.stopped).toContain('could not apply 91e605d');
  });

  it('drops it once the operation finishes', async () => {
    const merge = loaded();
    vi.spyOn(commands, 'operationStep').mockResolvedValue({
      completed: true,
      state: 'clean',
      conflicts: [],
      message: '',
    });

    merge.stopped = 'error: could not apply 91e605d... local: dummy1 file';
    expect(await merge.step('continue')).toBe(true);
    expect(merge.stopped).toBe('');
  });
});
