import {
  conflictBlocks,
  operationStep,
  repoConflicts,
  repoOperation,
  resolveConflict,
  type Choice,
} from '../ipc/commands';
import type { Block, Blocks, ConflictedFile, Operation } from '../ipc/types';

/** Which side a conflicting region should take. */
export type Side = 'ours' | 'theirs' | 'base';

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
  /** Chosen side per conflicting block, by its index among the conflicts. */
  choices = $state<Record<number, Side>>({});
  busy = $state(false);
  error = $state<string | null>(null);

  #path = '';

  /** True while git is mid-merge, mid-rebase, or otherwise stopped. */
  inProgress = $derived(this.operation !== null && this.operation.state !== 'clean');

  /** Conflicting regions of the open file, in order. */
  conflicts = $derived((this.blocks?.blocks ?? []).filter((b) => b.kind === 'conflict'));

  /** True once every conflicting region of the open file has been decided. */
  settled = $derived(
    this.conflicts.length > 0 &&
      this.conflicts.every((_, i) => this.choices[i] !== undefined),
  );

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    try {
      const [operation, files] = await Promise.all([repoOperation(path), repoConflicts(path)]);
      this.operation = operation;
      this.files = files;
      if (this.active !== null && !files.some((f) => f.path === this.active)) this.close();
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  async open(file: string): Promise<void> {
    this.active = file;
    this.blocks = null;
    this.choices = {};
    this.error = null;
    try {
      this.blocks = await conflictBlocks(this.#path, file);
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  close(): void {
    this.active = null;
    this.blocks = null;
    this.choices = {};
  }

  choose(index: number, side: Side): void {
    this.choices = { ...this.choices, [index]: side };
  }

  /** Takes one side for every region at once, which is how most conflicts are settled. */
  chooseAll(side: Side): void {
    const all: Record<number, Side> = {};
    this.conflicts.forEach((_, i) => (all[i] = side));
    this.choices = all;
  }

  /** Writes the decisions and stages the file. */
  async apply(): Promise<boolean> {
    const file = this.active;
    if (file === null || this.blocks === null) return false;
    return this.run(file, { kind: 'content', text: render(this.blocks.blocks, this.choices) });
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
      this.error = e instanceof Error ? e.message : String(e);
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
      await this.#reload();
      return out.completed;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
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
export function render(blocks: readonly Block[], choices: Record<number, Side>): string {
  const out: string[] = [];
  let conflict = 0;
  for (const block of blocks) {
    if (block.kind === 'common') {
      out.push(...block.lines);
      continue;
    }
    const side = choices[conflict] ?? 'ours';
    conflict += 1;
    out.push(...(side === 'ours' ? block.ours : side === 'theirs' ? block.theirs : block.base));
  }
  // A trailing newline, because every line git handed over was one line of a text file.
  return out.length === 0 ? '' : `${out.join('\n')}\n`;
}
