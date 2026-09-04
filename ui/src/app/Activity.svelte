<script lang="ts">
  import type { ActivityState, Channel } from '../state/activity.svelte';
  import type { ActivityEntry } from '../ipc/activity';

  const { activity, onClose }: { activity: ActivityState; onClose: () => void } = $props();

  const tabs: { id: Channel; label: string; hint: string }[] = [
    {
      id: 'app',
      label: 'Application',
      hint: 'What Coral itself has been doing, and every warning it has raised',
    },
    {
      id: 'repo',
      label: 'Repository',
      hint: 'Every operation Coral has run against the repository that is open',
    },
  ];

  function time(at: number): string {
    return new Date(at).toLocaleTimeString();
  }

  /**
   * How long an operation took, only where it is worth reading.
   *
   * Under a tenth of a second is every command that did nothing much, and a column of `2ms`
   * down the side of the log buries the one entry that took eleven seconds.
   */
  function took(entry: ActivityEntry): string {
    if (entry.millis === null || entry.millis < 100) return '';
    return entry.millis < 1000 ? `${entry.millis}ms` : `${(entry.millis / 1000).toFixed(1)}s`;
  }
</script>

<div class="sheet">
  <header>
    <h2>Activity logs</h2>
    <button class="shut" title="Close" onclick={onClose}>✕</button>
  </header>

  <nav class="tabs">
    {#each tabs as tab (tab.id)}
      <button
        class="tab"
        class:on={activity.channel === tab.id}
        title={tab.hint}
        onclick={() => void activity.show(tab.id)}
      >{tab.label}</button>
    {/each}
    <span class="spacer"></span>
    <button class="act" onclick={() => void activity.load()}>Refresh</button>
    <button class="act" onclick={() => void activity.clear()}>Clear</button>
  </nav>

  {#if activity.error}
    <p class="note error">{activity.error}</p>
  {:else if activity.loading && activity.entries.length === 0}
    <p class="note">Reading the log…</p>
  {:else if activity.entries.length === 0}
    <p class="note">
      {activity.channel === 'repo'
        ? 'Nothing yet. Operations on this repository appear here as they run.'
        : 'Nothing yet. Coral records what it is doing here, warnings first.'}
    </p>
  {:else}
    <!-- Newest last, as a log reads: the interesting end of one being investigated is the end
         it is still being written to. -->
    <ol class="lines">
      {#each activity.entries as entry, i (`${entry.at}:${i}`)}
        <li class={entry.level}>
          <span class="at">{time(entry.at)}</span>
          <span class="what">{entry.message}</span>
          <span class="took">{took(entry)}</span>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  /* Covers the panes, as the other full-window panels do. A log is read by scrolling, and a
     floating dialog small enough to leave the graph visible is too small to scroll usefully. */
  .sheet {
    position: absolute; inset: 0; z-index: 30;
    display: flex; flex-direction: column;
    background: var(--bg-1); color: var(--fg-0);
  }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
  }
  h2 { margin: 0; font-size: 14px; font-weight: 600; flex: 1; }
  .shut {
    font: inherit; font-size: 18px; line-height: 1; cursor: pointer;
    background: var(--bg-1); border: 0; color: var(--fg-2); padding: 0 var(--space-2);
  }
  .shut:hover { color: var(--fg-0); }

  .tabs {
    display: flex; align-items: center; gap: var(--space-1);
    padding: 0 var(--space-4); border-bottom: 1px solid var(--border);
  }
  .spacer { flex: 1; }
  .tab {
    font: inherit; font-size: 12px; font-weight: 600; cursor: pointer;
    padding: var(--space-2) var(--space-3);
    background: var(--bg-1); border: 0; border-bottom: 2px solid transparent;
    color: var(--fg-2);
  }
  .tab:hover { color: var(--fg-0); }
  .tab.on { color: var(--fg-0); border-bottom-color: var(--accent); }
  .act {
    font: inherit; font-size: 11px; cursor: pointer;
    padding: 2px var(--space-3); margin: var(--space-2) 0;
    background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    color: var(--fg-1);
  }
  .act:hover { background: var(--bg-3); color: var(--fg-0); }

  .note { margin: 0; padding: var(--space-4); color: var(--fg-2); font-size: 12px; }
  .note.error { color: var(--danger); }

  .lines {
    list-style: none; margin: 0; padding: var(--space-2) var(--space-4);
    flex: 1; min-height: 0; overflow-y: auto;
    font-family: var(--font-mono); font-size: 11px; line-height: 1.6;
  }
  /* Fixed columns rather than one wrapped line: the times line up, so the gaps between
     operations can be read down the column without reading the operations. */
  .lines li {
    display: grid; grid-template-columns: 7.5em 1fr auto; gap: var(--space-3);
    background: var(--bg-1); color: var(--fg-1);
  }
  .at { color: var(--fg-2); }
  .what { overflow-wrap: anywhere; }
  .took { color: var(--fg-2); font-variant-numeric: tabular-nums; }
  .lines li.warn .what { color: var(--warn); }
  .lines li.error .what { color: var(--danger); }
</style>
