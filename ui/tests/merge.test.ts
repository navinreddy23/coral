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

/** The lines of one side, as a pick names them: `take('ours', 0, 1)` is its first two. */
const take = (side: 'ours' | 'theirs' | 'base', ...lines: number[]) =>
  lines.map((line) => ({ side, line }));

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
    const out = render(blocks, { 0: take('theirs', 0), 1: take('ours', 0) });
    expect(out).toBe('top\nyours\nmiddle\nmine2\nend\n');
  });

  it('can take the base, which is how a region is reverted', () => {
    const blocks = [conflict(['mine'], ['yours'], ['original'])];
    expect(render(blocks, { 0: take('base', 0) })).toBe('original\n');
  });

  it('keeps the base where nobody has taken a side', () => {
    // The lines as they were before either branch touched them: the one answer that cannot
    // be said to favour either side, and what the result pane shows for an untouched region.
    const blocks = [conflict(['mine'], ['yours'], ['original'])];
    expect(render(blocks, {})).toBe('original\n');
  });

  it('goes back to the base when every line is taken back out', () => {
    // Unticking both sides is how a region is put back the way it was, which is the same
    // thing an untouched region does: there is one meaning for "nothing taken".
    const blocks = [common('a'), conflict(['mine'], ['yours'], ['was']), common('b')];
    expect(render(blocks, { 0: [] })).toBe('a\nwas\nb\n');
  });

  it('drops a region with no base when nothing is taken', () => {
    // Two branches that each added a file has no "before" to fall back to, so taking neither
    // side really does leave nothing.
    const blocks = [common('a'), conflict(['mine'], ['yours']), common('b')];
    expect(render(blocks, { 0: [] })).toBe('a\nb\n');
  });

  it('handles a side that contributes no lines', () => {
    // One side deleted the region entirely; taking it must not leave a blank line behind.
    const blocks = [common('a'), conflict([], ['yours']), common('b')];
    expect(render(blocks, { 0: take('ours', 0) })).toBe('a\nb\n');
  });

  it('produces nothing for a file whose every line was dropped', () => {
    expect(render([conflict([], ['yours'])], { 0: take('ours', 0) })).toBe('');
  });

  it('numbers decisions by conflict, not by block', () => {
    // The second conflict is index 1 even though it is the fourth block, so the first one is
    // untouched and keeps its base.
    const blocks = [
      common('a'),
      conflict(['x'], ['y'], ['was x']),
      common('b'),
      conflict(['p'], ['q'], ['was p']),
    ];
    expect(render(blocks, { 1: take('theirs', 0) })).toBe('a\nwas x\nb\nq\n');
  });
});

describe('taking lines out of a region', () => {
  /** Two lines on each side of one conflict, which is what line picking is for. */
  const two = [
    common('top'),
    conflict(['mine one', 'mine two'], ['yours one', 'yours two'], ['was one', 'was two']),
    common('end'),
  ];

  it('writes the sides in the order they were taken', () => {
    expect(render(two, { 0: [...take('theirs', 0, 1), ...take('ours', 0, 1)] })).toBe(
      'top\nyours one\nyours two\nmine one\nmine two\nend\n',
    );
  });

  it('takes one line from each side, which is the point of picking by line', () => {
    expect(render(two, { 0: [...take('ours', 0), ...take('theirs', 1)] })).toBe(
      'top\nmine one\nyours two\nend\n',
    );
  });

  it('keeps the lines in the order they were clicked, not in side order', () => {
    expect(render(two, { 0: [...take('theirs', 1), ...take('ours', 0)] })).toBe(
      'top\nyours two\nmine one\nend\n',
    );
  });

  it('can keep all three sides, base included', () => {
    const blocks = [conflict(['mine'], ['yours'], ['was'])];
    const picks = [...take('base', 0), ...take('ours', 0), ...take('theirs', 0)];
    expect(render(blocks, { 0: picks })).toBe('was\nmine\nyours\n');
  });

  it('ignores a line that is no longer there', () => {
    // Defensive: a pick outlives the blocks it was made against only if something reloaded
    // underneath, and half a file is worse than a line missing from it.
    expect(render(two, { 0: take('ours', 0, 9) })).toBe('top\nmine one\nend\n');
  });
});

describe('picking sides region by region', () => {
  /** A state with one file open and two conflicting regions in it. */
  function opened() {
    const merge = new MergeState();
    merge.blocks = {
      path: 'f.txt',
      blocks: [
        common('top'),
        conflict(['mine'], ['yours']),
        common('middle'),
        conflict(['mine2'], ['yours2']),
      ],
    };
    return merge;
  }

  it('takes a whole side, and takes it back out when it is picked again', () => {
    const merge = opened();
    merge.toggle(0, 'ours');
    merge.toggle(0, 'theirs');
    expect(merge.choices[0]).toEqual([
      { side: 'ours', line: 0 },
      { side: 'theirs', line: 0 },
    ]);
    merge.toggle(0, 'ours');
    expect(merge.choices[0]).toEqual([{ side: 'theirs', line: 0 }]);
  });

  it('takes a single line, leaving the rest of that side out', () => {
    const merge = new MergeState();
    merge.blocks = {
      path: 'f.txt',
      blocks: [conflict(['mine one', 'mine two'], ['yours one', 'yours two'])],
    };
    merge.toggleLine(0, 'theirs', 1);
    merge.toggleLine(0, 'ours', 0);

    expect(merge.output).toBe('yours two\nmine one\n');
    merge.toggleLine(0, 'theirs', 1);
    expect(merge.output).toBe('mine one\n');
  });

  it('counts the regions nobody has touched, since those keep the base', () => {
    const merge = opened();
    expect(merge.untouched).toBe(2);
    merge.toggle(0, 'ours');
    expect(merge.untouched).toBe(1);
    merge.toggle(1, 'theirs');
    expect(merge.untouched).toBe(0);
  });

  it('leaves a region taking nothing when its lines are all taken back out', () => {
    const merge = opened();
    merge.toggle(0, 'ours');
    merge.toggle(0, 'ours');
    merge.toggle(1, 'ours');
    expect(merge.choices[0]).toEqual([]);
    expect(merge.untouched).toBe(0);
    expect(merge.output).toBe('top\nmiddle\nmine2\n');
  });

  it('takes one side everywhere at once', () => {
    const merge = opened();
    merge.chooseAll('theirs');
    expect(merge.output).toBe('top\nyours\nmiddle\nyours2\n');
  });

  it('ends a hand-typed result with a newline, because a text file has one', async () => {
    const merge = opened();
    merge.active = 'f.txt';
    const sent: string[] = [];
    vi.spyOn(commands, 'resolveConflict').mockImplementation(async (_p, _f, choice) => {
      if (choice.kind === 'content') sent.push(choice.text);
    });
    vi.spyOn(commands, 'repoOperation').mockResolvedValue({
      state: 'merge',
      labels: { ours: 'a', theirs: 'b', swapped: false },
      progress: null,
      headName: null,
      stoppedAt: null,
      interactive: false,
    });
    vi.spyOn(commands, 'repoConflicts').mockResolvedValue([]);

    merge.edit('typed with no return at the end');
    await merge.apply();
    expect(sent).toEqual(['typed with no return at the end\n']);
  });

  it('hands the typed text back once the result is edited, and forgets it on request', () => {
    const merge = opened();
    merge.chooseAll('ours');
    merge.edit('something neither side wrote\n');
    expect(merge.output).toBe('something neither side wrote\n');
    // Typed over, the picks no longer decide anything, so none of them are outstanding.
    expect(merge.untouched).toBe(0);
    merge.unedit();
    expect(merge.output).toBe('top\nmine\nmiddle\nmine2\n');
  });
});

describe('arriving at a stopped operation', () => {
  /** The reads a load does, answered with `files` and a merge in progress. */
  function wire(files: { path: string; kind: string; binary: boolean; deleteModify: boolean }[]) {
    // Each case counts its own calls; the spies outlive the test that made them otherwise.
    vi.clearAllMocks();
    vi.spyOn(commands, 'repoOperation').mockResolvedValue({
      state: 'cherry_pick',
      labels: { ours: 'dummyx', theirs: '056d9dc (lets see what happens)', swapped: false },
      progress: null,
      headName: null,
      stoppedAt: null,
      interactive: false,
    });
    vi.spyOn(commands, 'repoConflicts').mockResolvedValue(files as never);
    vi.spyOn(commands, 'conflictBlocks').mockImplementation(async (_p, file) => ({
      blocks: [{ kind: 'conflict', base: ['base'], ours: [`${file} ours`], theirs: [`${file} theirs`] }],
    }) as never);
  }

  const conflicted = (path: string) => ({
    path,
    kind: 'both_modified',
    binary: false,
    deleteModify: false,
  });

  it('opens the first conflicted file rather than waiting to be asked', async () => {
    // The pane said "pick a file to settle it" and people did not know that was a control:
    // the report was that the region view did not work at all.
    wire([conflicted('dummy.txt')]);
    const merge = new MergeState();
    await merge.load('/repo');

    expect(merge.active).toBe('dummy.txt');
    expect(merge.blocks).not.toBeNull();
  });

  it('opens the next one when the file being worked on is settled', async () => {
    wire([conflicted('a.txt'), conflicted('b.txt')]);
    const merge = new MergeState();
    await merge.load('/repo');
    expect(merge.active).toBe('a.txt');

    // a.txt resolved: the reload sees only b.txt, and lands on it.
    wire([conflicted('b.txt')]);
    merge.close();
    await merge.load('/repo');
    expect(merge.active).toBe('b.txt');
  });

  it('leaves a file already open alone', async () => {
    wire([conflicted('a.txt'), conflicted('b.txt')]);
    const merge = new MergeState();
    await merge.load('/repo');
    await merge.open('b.txt');
    await merge.load('/repo');
    expect(merge.active).toBe('b.txt');
  });

  it('does not read blocks for a file that has none', async () => {
    // Binary, or on one side only: the whole-file choices are all there is, and the read
    // would be parsing a blob to show nothing.
    wire([{ path: 'logo.png', kind: 'both_modified', binary: true, deleteModify: false }]);
    const merge = new MergeState();
    await merge.load('/repo');

    expect(merge.active).toBe('logo.png');
    expect(merge.blocks).toBeNull();
    expect(commands.conflictBlocks).not.toHaveBeenCalled();
  });

  it('opens nothing when nothing is conflicted', async () => {
    wire([]);
    const merge = new MergeState();
    await merge.load('/repo');
    expect(merge.active).toBeNull();
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
