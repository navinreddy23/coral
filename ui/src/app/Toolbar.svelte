<script lang="ts">
  const { repo, branch, busy, onAction }: {
    repo: string;
    branch: string;
    busy: boolean;
    onAction: (name: string) => void;
  } = $props();

  /**
   * Grouped as the reference groups them: history, then the remote, then the working copy.
   * A rule is drawn between groups rather than extra space, because the toolbar has to stay
   * one row wide on a narrow window.
   */
  const groups = [
    [
      { name: 'undo', label: 'Undo', glyph: '↶', hint: 'Undo the last ref change' },
      { name: 'redo', label: 'Redo', glyph: '↷', hint: 'Redo what was undone' },
    ],
    [
      { name: 'fetch', label: 'Fetch', glyph: '⟳', hint: 'Fetch and prune' },
      { name: 'pull', label: 'Pull', glyph: '↓', hint: 'Pull, fast-forward only' },
      { name: 'push', label: 'Push', glyph: '↑', hint: 'Push, setting upstream if needed' },
    ],
    [
      { name: 'branch', label: 'Branch', glyph: '⑂', hint: 'Create a branch here' },
      { name: 'stash', label: 'Stash', glyph: '⤓', hint: 'Stash the working copy' },
      { name: 'pop', label: 'Pop', glyph: '⤒', hint: 'Apply the latest stash and drop it' },
    ],
  ];
</script>

<div class="toolbar">
  <div class="where">
    <span class="label">repository</span>
    <span class="value">{repo}</span>
    <span class="sep">›</span>
    <span class="label">branch</span>
    <span class="value">{branch}</span>
  </div>

  <div class="actions">
    {#each groups as group, i (i)}
      {#if i > 0}<span class="rule" aria-hidden="true"></span>{/if}
      {#each group as action (action.name)}
        <button
          class="action"
          disabled={busy}
          title={action.hint}
          onclick={() => onAction(action.name)}
        >
          <span class="glyph">{action.glyph}</span>
          <span class="name">{action.label}</span>
        </button>
      {/each}
    {/each}
  </div>
</div>

<style>
  .toolbar {
    display: flex; align-items: center; gap: var(--space-5);
    height: 46px; box-sizing: border-box; padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  .where { display: flex; align-items: baseline; gap: var(--space-2); min-width: 0; }
  .label {
    font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--fg-2);
  }
  .value {
    font-size: 13px; font-weight: 600; color: var(--fg-0);
    max-width: 16em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .sep { color: var(--fg-2); }

  .actions { display: flex; align-items: center; gap: 2px; margin: 0 auto; }
  .rule {
    width: 1px; height: 22px; margin: 0 var(--space-2);
    background: var(--border); flex: 0 0 auto;
  }
  .action {
    display: flex; flex-direction: column; align-items: center; gap: 2px;
    min-width: 54px; padding: 3px var(--space-2); line-height: 13px;
    font: inherit; cursor: pointer;
    background: none; border: 1px solid transparent; border-radius: var(--radius-1);
    color: var(--fg-1);
  }
  .action:hover:not(:disabled) {
    background: var(--bg-2); border-color: var(--border); color: var(--fg-0);
  }
  /* Pressed reads as pressed rather than as another hover: the surface goes under the page
     instead of above it. */
  .action:active:not(:disabled) { background: var(--bg-3); }
  .action:disabled { color: var(--fg-2); cursor: default; opacity: 0.5; }
  .name {
    font-size: 10px; text-transform: uppercase; letter-spacing: 0.04em;
  }
  .glyph { font-size: 15px; line-height: 1; color: var(--accent); }
  .action:disabled .glyph { color: inherit; }
</style>
