<script lang="ts">
  /**
   * Choosing which old branches to delete, all at once.
   *
   * Two lists, because they are two different decisions. A branch already merged into the one
   * checked out loses nothing by going, so those are ticked. A branch whose upstream was
   * deleted but which holds commits nothing else has is a branch somebody may still want, so
   * those are listed, unticked, under a warning; ticking one is choosing to lose its commits.
   */
  const {
    into,
    merged,
    gone,
    onDelete,
    onClose,
  }: {
    /** The branch the merged ones are merged into. */
    into: string;
    merged: string[];
    /** Branches whose upstream is gone and which are not merged. */
    gone: string[];
    onDelete: (merged: string[], unmerged: string[]) => void;
    onClose: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let keep = $state<Record<string, boolean>>(Object.fromEntries(gone.map((b) => [b, true])));
  // svelte-ignore state_referenced_locally
  let mergedOn = $state<Record<string, boolean>>(Object.fromEntries(merged.map((b) => [b, true])));

  const chosenMerged = $derived(merged.filter((b) => mergedOn[b]));
  const chosenGone = $derived(gone.filter((b) => !keep[b]));
  const count = $derived(chosenMerged.length + chosenGone.length);

  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
      event.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim" role="presentation" onclick={onClose}>
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-labelledby="cleanup-title"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => undefined}
  >
    <h2 id="cleanup-title">Clean up branches</h2>
    {#if merged.length === 0 && gone.length === 0}
      <p class="muted">
        Nothing to clean up: every other branch has commits {into} does not, and still follows
        an upstream that exists.
      </p>
    {/if}
    {#if merged.length > 0}
      <h3>Merged into {into}</h3>
      <p class="muted">Every commit on these is on {into} already, so deleting them loses nothing.</p>
      <ul>
        {#each merged as branch (branch)}
          <li>
            <label>
              <input type="checkbox" bind:checked={mergedOn[branch]} />
              <span class="name">{branch}</span>
            </label>
          </li>
        {/each}
      </ul>
    {/if}
    {#if gone.length > 0}
      <h3>Upstream deleted, not merged</h3>
      <p class="warn">
        These followed a branch that is gone from the remote, and hold commits {into} does not
        have. Tick one only if those commits are not wanted.
      </p>
      <ul>
        {#each gone as branch (branch)}
          <li>
            <label>
              <input
                type="checkbox"
                checked={!keep[branch]}
                onchange={(e) => (keep[branch] = !e.currentTarget.checked)}
              />
              <span class="name">{branch}</span>
            </label>
          </li>
        {/each}
      </ul>
    {/if}
    <p class="muted">
      Undo brings every one of them back. main, master, develop and trunk are never offered.
    </p>
    <div class="actions">
      <button class="cancel" onclick={onClose}>Cancel</button>
      <button
        class="danger"
        disabled={count === 0}
        onclick={() => onDelete(chosenMerged, chosenGone)}
      >
        {count === 1 ? 'Delete 1 branch' : `Delete ${count} branches`}
      </button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 30; background: var(--scrim);
    display: flex; justify-content: center; align-items: flex-start; padding-top: 14vh;
  }
  .sheet {
    width: min(480px, 90vw); max-height: 70vh; overflow-y: auto; padding: var(--space-4);
    background: var(--bg-0); color: var(--fg-1); border: 1px solid var(--border-strong);
    border-radius: var(--radius-3); box-shadow: var(--elevate-2);
  }
  h2 { margin: 0 0 var(--space-2); font-size: var(--text-lg); font-weight: var(--weight-semibold); color: var(--fg-0); }
  h3 { margin: var(--space-3) 0 var(--space-1); font-size: var(--text-base); font-weight: var(--weight-semibold); color: var(--fg-0); }
  p { margin: 0 0 var(--space-2); line-height: 1.5; font-size: var(--text-base); }
  .muted { color: var(--fg-2); }
  .warn { color: var(--warn); }
  ul { list-style: none; margin: 0 0 var(--space-2); padding: 0; }
  li label { display: flex; align-items: center; gap: var(--space-2); padding: 2px 0; }
  .name { font-family: var(--font-mono); font-variant-ligatures: none; color: var(--fg-0); }
  .actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-4); }
  button {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: var(--space-1) var(--space-3);
    background: var(--bg-1); border: 1px solid var(--border-strong);
    border-radius: var(--radius-1); color: var(--fg-1);
  }
  button:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  button:disabled { opacity: 0.5; cursor: default; }
  /* Outlined, as every dialog here sets the button that destroys something. */
  button.danger:not(:disabled) {
    color: var(--danger); border-color: var(--danger); background: var(--bg-0);
  }
  button.danger:hover:not(:disabled) { background: var(--danger-soft); color: var(--danger); }
  .cancel { margin-right: auto; border-color: transparent; background: var(--bg-0); }
</style>
