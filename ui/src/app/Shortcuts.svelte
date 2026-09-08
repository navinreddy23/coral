<script lang="ts">
  import { BINDINGS } from '../state/shortcuts';

  const { live, onClose }: { live: Set<string>; onClose: () => void } = $props();

  const groups = ['Repo actions', 'Navigation', 'Command palette', 'UI'] as const;
</script>

<!--
  On the window, as `Activity` and `Menu` do it. Hung on the scrim itself the handler needed
  the scrim to hold focus, which nothing ever gave it, so Escape did nothing and the sheet
  could only be dismissed by clicking it.
-->
<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<div class="scrim" role="presentation" onclick={onClose}>
  <div class="sheet" role="dialog" aria-label="Keyboard shortcuts">
    <h2>Keyboard shortcuts</h2>
    <p class="note">Dimmed entries are not wired up yet.</p>
    <div class="grid">
      {#each groups as group (group)}
        <section>
          <h3>{group}</h3>
          {#each BINDINGS.filter((b) => b.group === group) as b (b.id)}
            <div class="row" class:pending={!live.has(b.id)}>
              <span class="label">{b.label}</span>
              <kbd>{b.keys}</kbd>
            </div>
          {/each}
        </section>
      {/each}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 10; border: 0; padding: 0;
    display: flex; align-items: center; justify-content: center;
    background: rgb(0 0 0 / 45%);
  }
  .sheet {
    max-height: 82vh; overflow-y: auto; width: min(820px, 92vw);
    padding: var(--space-5); border-radius: 6px;
    background: var(--bg-0); border: 1px solid var(--border); color: var(--fg-0);
    text-align: left; cursor: default;
  }
  h2 { margin: 0 0 var(--space-1); font-size: var(--text-lg); }
  .note { margin: 0 0 var(--space-4); font-size: var(--text-base); color: var(--fg-2); }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-5); }
  h3 {
    font-size: var(--text-sm); text-transform: uppercase; letter-spacing: 0.05em;
    color: var(--fg-2); margin: 0 0 var(--space-2);
  }
  .row {
    display: flex; align-items: baseline; gap: var(--space-3);
    padding: 2px 0; font-size: var(--text-base);
  }
  .row.pending { opacity: 0.45; }
  .label { flex: 1; color: var(--fg-1); }
  kbd {
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); white-space: nowrap;
    padding: 1px var(--space-2); border-radius: 3px;
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-0);
  }
</style>
