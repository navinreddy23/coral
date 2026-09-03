<script lang="ts">
  const { repo, branch, busy, onAction }: {
    repo: string;
    branch: string;
    busy: boolean;
    onAction: (name: string) => void;
  } = $props();

  /**
   * Labels sit above their glyph, as in the reference. Actions that are not wired yet are
   * disabled rather than absent, so the shape of the toolbar does not shift as they land.
   */
  const actions = [
    { name: 'undo', label: 'Undo', glyph: '↶', ready: false },
    { name: 'redo', label: 'Redo', glyph: '↷', ready: false },
    { name: 'pull', label: 'Pull', glyph: '↓', ready: false },
    { name: 'push', label: 'Push', glyph: '↑', ready: false },
    { name: 'branch', label: 'Branch', glyph: '⑂', ready: false },
    { name: 'stash', label: 'Stash', glyph: '⤓', ready: false },
    { name: 'pop', label: 'Pop', glyph: '⤒', ready: false },
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
    {#each actions as action (action.name)}
      <button
        class="action"
        disabled={!action.ready || busy}
        title={action.ready ? action.label : `${action.label} is not wired up yet`}
        onclick={() => onAction(action.name)}
      >
        <span class="glyph">{action.glyph}</span>
        <span class="name">{action.label}</span>
      </button>
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
  .label { font-size: 10px; text-transform: uppercase; letter-spacing: 0.05em; color: var(--fg-2); }
  .value {
    font-size: 13px; color: var(--fg-0);
    max-width: 16em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .sep { color: var(--fg-2); }

  .actions { display: flex; gap: var(--space-1); margin: 0 auto; }
  .action {
    display: flex; flex-direction: column; align-items: center; gap: 1px;
    min-width: 52px; padding: 2px var(--space-2); line-height: 14px;
    font: inherit; cursor: pointer;
    background: none; border: 0; border-radius: 3px; color: var(--fg-1);
  }
  .action:hover:not(:disabled) { background: var(--bg-3); }
  .action:disabled { color: var(--fg-2); cursor: default; opacity: 0.55; }
  .name { font-size: 10px; }
  .glyph { font-size: 15px; line-height: 1; }
</style>
