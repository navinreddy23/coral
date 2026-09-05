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

/**
 * What one region resolves to: the sides taken, in the order they were taken.
 *
 * A list rather than one side, because plenty of conflicts are settled by keeping both — the
 * line they added and the line you added — and which goes first is part of the answer. An
 * empty list is a decision too: the region takes nothing and the lines go.
 */
export type Pick = Side[];

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

  #path = '';

  /** True while git is mid-merge, mid-rebase, or otherwise stopped. */
  inProgress = $derived(this.operation !== null && this.operation.state !== 'clean');

  /** Conflicting regions of the open file, in order. */
  conflicts = $derived((this.blocks?.blocks ?? []).filter((b) => b.kind === 'conflict'));

  /** True once every conflicting region of the open file has been decided. */
  settled = $derived(
    this.edited !== null ||
      (this.conflicts.length > 0 && this.conflicts.every((_, i) => this.choices[i] !== undefined)),
  );

  /** The file as it will be written: the picks applied, or whatever was typed over them. */
  output = $derived(
    this.edited ?? (this.blocks === null ? '' : render(this.blocks.blocks, this.choices)),
  );

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      const [operation, files] = await Promise.all([repoOperation(path), repoConflicts(path)]);
      this.operation = operation;
      this.files = files;
      if (operation.state === 'clean') this.stopped = '';
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
   * Adds a side to a region, or takes it back out.
   *
   * Picking is a toggle rather than a choice of one, so that both sides can be kept: click
   * theirs then ours and the region is written in that order.
   */
  toggle(index: number, side: Side): void {
    const at = this.choices[index] ?? [];
    const next = at.includes(side) ? at.filter((s) => s !== side) : [...at, side];
    this.choices = { ...this.choices, [index]: next };
  }

  /** Takes one side and nothing else for a region. */
  choose(index: number, side: Side): void {
    this.choices = { ...this.choices, [index]: [side] };
  }

  /** Takes one side for every region at once, which is how most conflicts are settled. */
  chooseAll(side: Side): void {
    const all: Record<number, Pick> = {};
    this.conflicts.forEach((_, i) => (all[i] = [side]));
    this.choices = all;
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
      this.stopped = out.completed ? '' : out.message;
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
 * Rebuilds the file from the blocks and the decisions.
 *
 * A region with no decision keeps our side, which is what git already put in the index — so a
 * partially decided file that is applied anyway is no worse than not having opened it.
 */
export function render(blocks: readonly Block[], choices: Record<number, Pick>): string {
  const out: string[] = [];
  let conflict = 0;
  for (const block of blocks) {
    if (block.kind === 'common') {
      out.push(...block.lines);
      continue;
    }
    const pick = choices[conflict] ?? ['ours'];
    conflict += 1;
    for (const side of pick) {
      out.push(...(side === 'ours' ? block.ours : side === 'theirs' ? block.theirs : block.base));
    }
  }
  // A trailing newline, because every line git handed over was one line of a text file.
  return out.length === 0 ? '' : `${out.join('\n')}\n`;
}
