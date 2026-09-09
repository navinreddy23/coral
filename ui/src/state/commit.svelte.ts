import type { WorktreeState } from './worktree.svelte';

/**
 * The commit being written.
 *
 * Held here rather than inside the staging panel, for two reasons. The panel is unmounted the
 * moment the selection moves off the working copy, so a half-written message was lost by
 * clicking a commit to check what it said — which is exactly when somebody looks. And a
 * keyboard shortcut has to be able to reach it: Ctrl+Enter commits, and neither the shortcut
 * nor the window could see a field that lived inside the panel.
 */
export class CommitState {
  summary = $state('');
  description = $state('');
  /** Whether this replaces the last commit rather than adding one. */
  amend = $state(false);

  /**
   * Bumped to ask the panel to put the caret in the summary field.
   *
   * A counter rather than a flag, so asking twice in a row asks twice: the panel reacts to the
   * value changing, and a flag set to true while already true is not a change.
   */
  focusTick = $state(0);

  /** Subject, blank line, body — which is the shape every git tool expects. */
  message = $derived(
    this.description.trim() === ''
      ? this.summary.trim()
      : `${this.summary.trim()}\n\n${this.description.trim()}`,
  );

  /** Whether there is something to record. Amending needs no staged file; committing does. */
  ready(worktree: WorktreeState): boolean {
    return (
      this.summary.trim().length > 0 &&
      (worktree.staged.length > 0 || this.amend) &&
      !worktree.busy
    );
  }

  /**
   * The message an amend was filled in with, so unticking can take back what it put there
   * without taking away anything the user typed on top of it.
   */
  #seeded: string | null = null;

  /** Fills the draft in from a commit's own message, which is where an amend starts. */
  seed(summary: string, body: string): void {
    this.summary = summary;
    this.description = body.trim();
    this.#seeded = this.message;
  }

  /**
   * Fills the draft in from the message git prepared for the next commit.
   *
   * Not [`seed`]: that message is put there by ticking amend and taken away by unticking it,
   * where this one is the starting point for a commit that is going to be made either way.
   */
  prepare(message: string): void {
    const [summary, ...rest] = message.split('\n');
    this.summary = summary ?? '';
    this.description = rest.join('\n').trim();
    this.#prepared = this.message;
  }

  /**
   * Takes a prepared message back when the operation it belonged to is over.
   *
   * A merge finished from the conflict tool never passes through this panel, so nothing
   * cleared the draft and the box was left holding "Merge branch 'side'" over a commit that
   * had already been made. Left alone if it has been edited since: that is somebody's own
   * message now, whatever it started as.
   */
  withdraw(): void {
    if (this.#prepared !== null && this.message === this.#prepared) {
      this.summary = '';
      this.description = '';
    }
    this.#prepared = null;
  }

  /** What `prepare` put there, so it can be told from something typed over it. */
  #prepared: string | null = null;

  /** Empties a seeded message again, unless it has been edited since it was put there. */
  unseed(): void {
    if (this.#seeded !== null && this.message === this.#seeded) {
      this.summary = '';
      this.description = '';
    }
    this.#seeded = null;
  }

  focus(): void {
    this.focusTick += 1;
  }

  clear(): void {
    this.summary = '';
    this.description = '';
    this.amend = false;
    this.#seeded = null;
    this.#prepared = null;
  }
}
