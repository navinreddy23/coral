import { rebaseStart, rebaseTodo } from '../ipc/commands';
import type { Step, Todo, TodoItem } from '../ipc/types';
import { messageOf } from '../ipc/error';

/**
 * An interactive rebase being composed.
 *
 * Nothing is started until the user says so: the list is built by reading the range, not by
 * beginning a rebase and reading the file git writes, so backing out costs nothing and leaves
 * no rebase to abort.
 */
export class RebaseState {
  /** The revision being rebased onto, or null when the picker is closed. */
  onto = $state<string | null>(null);
  items = $state<TodoItem[]>([]);
  loading = $state(false);
  busy = $state(false);
  error = $state<string | null>(null);

  #path = '';

  open = $derived(this.onto !== null);

  /**
   * Why the list would be refused, or null when it is ready to run.
   *
   * Whether a squash has anything to fold into is a question about the first commit git keeps,
   * not the first row: dropping the row above a squash leaves the squash first, and git stops
   * with "cannot 'squash' without a previous commit" — halfway in, with the branch detached and
   * a todo file to repair. Only the topmost row was ever checked.
   */
  whyNotReady = $derived.by(() => {
    const first = this.items.find((i) => i.step !== 'drop');
    if (first?.step === 'squash' || first?.step === 'fixup') {
      return 'The first commit kept has nothing above it to fold into.';
    }
    if (this.items.some((i) => i.step === 'reword' && (i.message ?? '').trim() === '')) {
      return 'A reworded commit needs a message.';
    }
    return null;
  });

  /** True when the list would be refused. */
  invalid = $derived(this.whyNotReady !== null);

  /** Commits that will survive, for the summary line. */
  remaining = $derived(this.items.filter((i) => i.step !== 'drop').length);

  async load(path: string, onto: string): Promise<void> {
    this.#path = path;
    this.onto = onto;
    this.items = [];
    this.error = null;
    this.loading = true;
    try {
      const todo = await rebaseTodo(path, onto);
      this.items = todo.items;
      if (todo.items.length === 0) this.error = `Nothing to rebase onto ${onto}.`;
    } catch (e) {
      this.error = messageOf(e);
    } finally {
      this.loading = false;
    }
  }

  close(): void {
    this.onto = null;
    this.items = [];
    this.error = null;
  }

  setStep(index: number, step: Step): void {
    this.items = this.items.map((item, i) => {
      if (i !== index) return item;
      // A reword starts from the message it has, since it is usually an edit of it rather
      // than a replacement; anything else has no message to carry.
      const message =
        step === 'reword' ? (item.message ?? item.summary) : null;
      return { ...item, step, message };
    });
  }

  /** The replacement message for a reworded commit. */
  setMessage(index: number, message: string): void {
    this.items = this.items.map((item, i) => (i === index ? { ...item, message } : item));
  }

  /** Moves one commit, keeping the rest in order. */
  move(from: number, to: number): void {
    if (from === to || from < 0 || to < 0) return;
    if (from >= this.items.length || to >= this.items.length) return;
    const next = [...this.items];
    const [moved] = next.splice(from, 1);
    if (moved === undefined) return;
    next.splice(to, 0, moved);
    this.items = next;
  }

  /**
   * Starts the rebase. Returns whether it finished without stopping.
   *
   * Refuses a list git would fail partway through, which would leave a rebase to abort rather
   * than the branch where it was.
   */
  async start(): Promise<boolean> {
    const onto = this.onto;
    if (onto === null || this.invalid) return false;
    this.busy = true;
    this.error = null;
    try {
      const todo: Todo = { items: this.items };
      const outcome = await rebaseStart(this.#path, onto, todo);
      this.close();
      return !outcome.conflicted;
    } catch (e) {
      this.error = messageOf(e);
      return false;
    } finally {
      this.busy = false;
    }
  }
}
