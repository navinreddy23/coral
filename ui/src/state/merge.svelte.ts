import {
  conflictBlocks,
  operationStep,
  repoConflicts,
  repoOperation,
  resolveConflict,
  type Choice,
} from '../ipc/commands';
import type { Block, Blocks, ConflictedFile, Operation } from '../ipc/types';
import { messageOf } from '../ipc/error';

/** Which side a conflicting region can take. */
export type Side = 'ours' | 'theirs' | 'base';

/** One line of one side, taken into the result. */
export interface Take {
  side: Side;
  /** Position within that side of the region, from zero. */
  line: number;
}

/**
 * What one region resolves to: the lines taken, in the order they were taken.
 *
 * Lines rather than sides, because a conflict is often settled by keeping one line of theirs
 * and one of yours out of a region that holds several — and the order they go in is part of
 * the answer. An empty list is a decision too: take nothing, and the region goes. No entry at
 * all is the region nobody has answered yet.
 */
export type Pick = Take[];

/**
 * The merge tool: what is in progress, what still conflicts, and the decisions made so far.
 *
 * Decisions live here rather than being written to the file as they are made, so a file is
 * only ever written once, when it is marked resolved. A half-applied file on disk is exactly
 * what makes a merge tool frightening to use.
 */
export class MergeState {
  operation = $state<Operation | null>(null);
  files = $state<ConflictedFile[]>([]);
  /** Path being worked on. */
  active = $state<string | null>(null);
  blocks = $state<Blocks | null>(null);
  /** What each conflicting block takes, by its index among the conflicts. */
  choices = $state<Record<number, Pick>>({});
  /**
   * The result typed out by hand, once someone starts editing it.
   *
   * Null while the picks alone decide the file. Two sides picked in order settles most
   * conflicts but not all: sometimes the answer is a line neither side wrote.
   */
  edited = $state<string | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  /**
   * Why the operation stopped, in git's own words.
   *
   * A rebase stops once per conflicting commit, so continuing often lands on the next one
   * rather than finishing. Without this the window looked identical either way — the file list
   * simply refilled — and nothing said which commit had failed to apply.
   */
  stopped = $state('');

  /**
   * True when the stop is one that continuing cannot answer.
   *
   * Kept as its own answer rather than read back off [`stopped`](#stopped), which has already
   * been reworded by then — asking the rewritten text whether it is git's original is a
   * question that can only say no.
   */
  nothingToRecord = $state(false);

  #path = '';

  /** True while git is mid-merge, mid-rebase, or otherwise stopped. */
  inProgress = $derived(this.operation !== null && this.operation.state !== 'clean');

  /** Conflicting regions of the open file, in order. */
  conflicts = $derived((this.blocks?.blocks ?? []).filter((b) => b.kind === 'conflict'));

  /** How many conflicting regions nobody has taken a side on yet. */
  untouched = $derived(
    this.edited === null
      ? this.conflicts.filter((_, i) => this.choices[i] === undefined).length
      : 0,
  );

  /** What the two sides are called, which is what the markers on an unanswered region carry. */
  sideNames = $derived({
    ours: this.operation?.labels.ours ?? 'ours',
    theirs: this.operation?.labels.theirs ?? 'theirs',
  });

  /**
   * True when every region has an answer, which is what makes the file safe to write.
   *
   * Typing the result by hand answers all of them at once: what is in the box is the file.
   */
  settled = $derived(this.edited !== null || this.untouched === 0);

  /** The file as it will be written: the picks applied, or whatever was typed over them. */
  output = $derived(
    this.edited ??
      (this.blocks === null ? '' : render(this.blocks.blocks, this.choices, this.sideNames)),
  );

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      const [operation, files] = await Promise.all([repoOperation(path), repoConflicts(path)]);
      this.operation = operation;
      this.files = files;
      if (operation.state === 'clean') {
        this.stopped = '';
        this.nothingToRecord = false;
      }
      if (this.active !== null && !files.some((f) => f.path === this.active)) this.close();
    } catch (e) {
      this.error = messageOf(e);
      return;
    }

    // Straight into the first file, rather than onto an empty pane asking for a file to be
    // picked. A stopped operation has exactly one thing to do next, and one conflicted file
    // is the common case; when there are several, resolving one opens the next.
    const first = this.files[0];
    if (this.active === null && first !== undefined) await this.open(first.path);
  }

  async open(file: string): Promise<void> {
    this.active = file;
    this.blocks = null;
    this.choices = {};
    this.edited = null;
    this.error = null;

    // A binary file, or one that exists on only one side, has nothing to pick between. The
    // window offers the whole-file choices for it, so reading blocks would mean parsing a
    // blob to show nothing — and for a binary one, parsing it as text at all.
    const known = this.files.find((f) => f.path === file);
    if (known && (known.binary || known.deleteModify)) return;
    try {
      this.blocks = await conflictBlocks(this.#path, file);
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  close(): void {
    this.active = null;
    this.blocks = null;
    this.choices = {};
    this.edited = null;
  }

  /**
   * Takes one line into the result, or takes it back out.
   *
   * A toggle rather than a choice of one, so any combination of the two sides can be kept:
   * their first line and your second, in the order they were clicked.
   */
  toggleLine(index: number, side: Side, line: number): void {
    const at = this.choices[index] ?? [];
    const has = at.some((t) => t.side === side && t.line === line);
    const next = has
      ? at.filter((t) => !(t.side === side && t.line === line))
      : [...at, { side, line }];
    this.choices = { ...this.choices, [index]: next };
  }

  /** Takes a whole side of one region, or takes all of it back out. */
  toggle(index: number, side: Side): void {
    const lines = this.linesOf(index, side);
    const at = this.choices[index] ?? [];
    const whole = lines.length > 0 && lines.every((_, i) => at.some((t) => t.side === side && t.line === i));
    const without = at.filter((t) => t.side !== side);
    const next = whole ? without : [...at, ...lines.map((_, i) => ({ side, line: i }))];
    this.choices = { ...this.choices, [index]: next };
  }

  /** Takes one side and nothing else for a region. */
  choose(index: number, side: Side): void {
    const lines = this.linesOf(index, side);
    this.choices = { ...this.choices, [index]: lines.map((_, i) => ({ side, line: i })) };
  }

  /** Takes one side for every region at once, which is how most conflicts are settled. */
  chooseAll(side: Side): void {
    const all: Record<number, Pick> = {};
    this.conflicts.forEach((block, i) => {
      const lines = block.kind === 'conflict' ? sideOf(block, side) : [];
      all[i] = lines.map((_, at) => ({ side, line: at }));
    });
    this.choices = all;
  }

  /** The lines one side holds in one region. */
  private linesOf(index: number, side: Side): readonly string[] {
    const block = this.conflicts[index];
    return block !== undefined && block.kind === 'conflict' ? sideOf(block, side) : [];
  }

  /** Starts editing the result by hand, seeded with what the picks produce. */
  edit(text: string): void {
    this.edited = text;
  }

  /** Goes back to the picks, discarding anything typed. */
  unedit(): void {
    this.edited = null;
  }

  /** Writes the decisions and stages the file. */
  async apply(): Promise<boolean> {
    const file = this.active;
    if (file === null || this.blocks === null) return false;
    // Never write a file that still has a question in it. Before this the unanswered regions
    // were written as the base, which resolved the file to the lines from before either
    // branch touched them: the merge went through and both sides' work on those lines was
    // gone, with nothing on screen having said so.
    if (!this.settled) {
      const n = this.untouched;
      this.error = `${n} conflict${n === 1 ? '' : 's'} in this file still needs a side taken.`;
      return false;
    }
    // A text file ends with a newline. The picks produce one; a box typed into only does when
    // the person happened to press return last, and dropping it rewrites the final line for
    // everyone who reads the file afterwards.
    const text =
      this.output === '' || this.output.endsWith('\n') ? this.output : `${this.output}\n`;
    return this.run(file, { kind: 'content', text });
  }

  /** Settles a file wholesale, without opening it. */
  async take(file: string, choice: Choice): Promise<boolean> {
    return this.run(file, choice);
  }

  async #reload(): Promise<void> {
    await this.load(this.#path);
  }

  private async run(file: string, choice: Choice): Promise<boolean> {
    this.busy = true;
    this.error = null;
    try {
      await resolveConflict(this.#path, file, choice);
      if (this.active === file) this.close();
      await this.#reload();
      return true;
    } catch (e) {
      this.error = messageOf(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  /** Continues, aborts, or skips the operation. */
  async step(step: 'continue' | 'abort' | 'skip'): Promise<boolean> {
    this.busy = true;
    this.error = null;
    try {
      const out = await operationStep(this.#path, step);
      this.nothingToRecord = !out.completed && emptied(out.message);
      this.stopped = out.completed ? '' : plainly(out.message);
      await this.#reload();
      return out.completed;
    } catch (e) {
      this.error = messageOf(e);
      await this.#reload();
      return false;
    } finally {
      this.busy = false;
    }
  }
}

/**
 * What an unanswered region is written as: git's own conflict markers.
 *
 * Not the base. Taking the lines from before either branch touched them reads as neutral and
 * is not: it throws away what both sides did there while looking like a resolution. Markers
 * say the one true thing about the region, which is that nobody has decided it yet.
 */
export function markersFor(
  block: Block & { kind: 'conflict' },
  labels: { ours: string; theirs: string },
): string[] {
  return [
    `<<<<<<< ${labels.ours}`,
    ...block.ours,
    '=======',
    ...block.theirs,
    `>>>>>>> ${labels.theirs}`,
  ];
}

/**
 * Rebuilds the file from the blocks and the decisions.
 *
 * A region with no entry is one nobody has answered, and it comes out as markers. A region
 * with an empty entry has been answered — take nothing — and comes out as nothing.
 */
export function render(
  blocks: readonly Block[],
  choices: Record<number, Pick>,
  labels: { ours: string; theirs: string } = { ours: 'ours', theirs: 'theirs' },
): string {
  const out: string[] = [];
  let conflict = 0;
  for (const block of blocks) {
    if (block.kind === 'common') {
      out.push(...block.lines);
      continue;
    }
    const pick = choices[conflict];
    conflict += 1;
    if (pick === undefined) {
      out.push(...markersFor(block, labels));
      continue;
    }
    for (const take of pick) {
      const line = sideOf(block, take.side)[take.line];
      if (line !== undefined) out.push(line);
    }
  }
  // A trailing newline, because every line git handed over was one line of a text file.
  return out.length === 0 ? '' : `${out.join('\n')}\n`;
}

/** The lines one side of a conflicting region holds. */
export function sideOf(block: Block & { kind: 'conflict' }, side: Side): readonly string[] {
  return side === 'ours' ? block.ours : side === 'theirs' ? block.theirs : block.base;
}

/**
 * A cherry-pick or revert whose change is already here ends up with nothing to record.
 *
 * git answers that with two commands to type — `git commit --allow-empty` to keep an empty
 * commit, or `--skip` to move on. This window has a button for the second, a button for
 * abandoning the whole thing, and no terminal in the way, so the advice names the one thing
 * the reader cannot do here and hides the two they can.
 */
export function plainly(message: string): string {
  if (!/is now empty/iu.test(message)) return message;
  return (
    'There is nothing left to record: this change is already here. Skip commit moves past it, ' +
    'and Abort undoes the whole thing.'
  );
}

/** True when the stop is one that continuing cannot answer. */
export function emptied(message: string): boolean {
  return /is now empty/iu.test(message);
}
