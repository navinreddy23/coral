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

  focus(): void {
    this.focusTick += 1;
  }

  clear(): void {
    this.summary = '';
    this.description = '';
    this.amend = false;
  }
}
