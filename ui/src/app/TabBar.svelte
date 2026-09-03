<script lang="ts">
  import { TabsState, type Tab } from '../state/tabs.svelte';

  const { tabs, onOpen }: { tabs: TabsState; onOpen: () => void } = $props();

  /** A group is a coloured band around a contiguous run of tabs, as Chrome draws them. */
  function bandColour(colour: string | undefined): string {
    return colour ? `var(--${colour.replace('lane', 'lane-')})` : 'transparent';
  }

  function title(tab: Tab): string {
    return TabsState.title(tab);
  }
</script>

<nav class="bar">
  {#each tabs.bands as band (band.group?.id ?? `loose-${band.tabs[0]?.id}`)}
    {#if band.group}
      {@const group = band.group}
      <div class="band" style:--band="{bandColour(group.colour)}">
        <button
          class="group"
          onclick={() => tabs.setCollapsed(group.id, !group.collapsed)}
          title={group.collapsed ? 'Expand group' : 'Collapse group'}
        >
          {group.name}
          {#if group.collapsed}<span class="tally">{band.tabs.length}</span>{/if}
        </button>
        {#if !group.collapsed}
          {#each band.tabs as tab (tab.id)}
            <div class="tab" class:active={tabs.session.active === tab.id} class:missing={tab.missing}>
              <button class="pick" onclick={() => tabs.activate(tab.id)} title={tab.path}>
                {title(tab)}
              </button>
              <button class="shut" onclick={() => tabs.close(tab.id)} title="Close">×</button>
            </div>
          {/each}
        {/if}
      </div>
    {:else}
      {#each band.tabs as tab (tab.id)}
        <div class="tab" class:active={tabs.session.active === tab.id} class:missing={tab.missing}>
          <button class="pick" onclick={() => tabs.activate(tab.id)} title={tab.path}>
            {title(tab)}
          </button>
          <button class="shut" onclick={() => tabs.close(tab.id)} title="Close">×</button>
        </div>
      {/each}
    {/if}
  {/each}

  <button class="add" onclick={onOpen} title="Open a repository">+</button>
  {#if tabs.error}<span class="error">{tabs.error}</span>{/if}
</nav>

<style>
  .bar {
    display: flex; align-items: stretch; gap: var(--space-1);
    padding: var(--space-1) var(--space-2) 0;
    background: var(--bg-2); border-bottom: 1px solid var(--border);
    overflow-x: auto;
  }
  .band {
    display: flex; align-items: stretch; gap: var(--space-1);
    padding: 0 var(--space-1) 2px;
    border-bottom: 2px solid var(--band);
  }
  .group {
    font: inherit; font-size: 11px; cursor: pointer; white-space: nowrap;
    padding: 0 var(--space-2); border: 0; border-radius: 3px 3px 0 0;
    background: var(--band); color: var(--bg-0);
  }
  .tally { opacity: 0.8; margin-left: var(--space-1); }

  .tab {
    display: flex; align-items: center; position: relative;
    border-radius: var(--radius-1) var(--radius-1) 0 0; background: transparent;
  }
  /*
   * The active tab is the page continuing upward: same surface, and a line of accent along
   * its top edge. Its own bottom border is covered by the bar's, which is what joins it to
   * the window below rather than leaving it floating in the strip.
   */
  .tab.active { background: var(--bg-0); }
  .tab.active::before {
    content: ''; position: absolute; inset: 0 0 auto; height: 2px;
    background: var(--accent); border-radius: var(--radius-1) var(--radius-1) 0 0;
  }
  .tab:hover:not(.active) { background: var(--bg-3); }
  .tab.missing .pick { text-decoration: line-through; color: var(--fg-2); }

  .pick {
    font: inherit; font-size: 12px; cursor: pointer; white-space: nowrap;
    max-width: 14em; overflow: hidden; text-overflow: ellipsis;
    padding: var(--space-2) var(--space-1) var(--space-2) var(--space-3);
    background: none; border: 0; color: var(--fg-1);
  }
  .tab.active .pick { color: var(--fg-0); }
  /* The close button appears on the tab being pointed at, and on the active one always: a row
     of crosses is noise, and a tab with no visible way to close it is a trap. */
  .shut {
    font: inherit; cursor: pointer; padding: 0 var(--space-2);
    background: none; border: 0; color: var(--fg-2); align-self: stretch;
    visibility: hidden;
  }
  .tab:hover .shut, .tab.active .shut { visibility: visible; }
  .shut:hover { color: var(--danger); }

  .add {
    font: inherit; font-size: 15px; line-height: 1; cursor: pointer; align-self: center;
    padding: 3px var(--space-2); margin-left: var(--space-1);
    background: none; border: 0; border-radius: var(--radius-1); color: var(--fg-2);
  }
  .add:hover { color: var(--fg-0); background: var(--bg-3); }
  .error { align-self: center; color: var(--danger); font-size: 11px; }
</style>
