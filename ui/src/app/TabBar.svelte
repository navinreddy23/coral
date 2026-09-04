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

  /** The tab being dragged, and what it is currently over. */
  let dragging = $state<number | null>(null);
  let insertBefore = $state<number | null>(null);
  let joining = $state<number | null>(null);
  let overBar = $state(false);

  function start(event: DragEvent, id: number) {
    dragging = id;
    // A plain-text payload as well, so a drop into another application gets the path rather
    // than nothing.
    const tab = tabs.session.tabs.find((t) => t.id === id);
    event.dataTransfer?.setData('text/plain', tab?.path ?? '');
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  }

  function clear() {
    dragging = null;
    insertBefore = null;
    joining = null;
    overBar = false;
  }

  /** Marks a drop target. Preventing the default is what makes one. */
  function allow(event: DragEvent): boolean {
    if (dragging === null) return false;
    event.preventDefault();
    event.stopPropagation();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    return true;
  }

  function overTab(event: DragEvent, id: number) {
    if (!allow(event) || dragging === id) return;
    insertBefore = id;
    joining = null;
    overBar = false;
  }

  function leaveTab(id: number) {
    if (insertBefore === id) insertBefore = null;
  }

  function overBand(event: DragEvent, group: number) {
    if (!allow(event)) return;
    joining = group;
    overBar = false;
  }

  function leaveBand(group: number) {
    if (joining === group) joining = null;
  }

  function overLoose(event: DragEvent) {
    if (dragging === null) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    overBar = true;
  }

  function dropOnTab(event: DragEvent, tab: Tab) {
    event.preventDefault();
    event.stopPropagation();
    const moved = dragging;
    clear();
    // In front of it, and into whatever group it belongs to.
    if (moved !== null && moved !== tab.id) void tabs.move(moved, tab.group, tab.id);
  }

  function dropOnBand(event: DragEvent, group: number) {
    event.preventDefault();
    event.stopPropagation();
    const moved = dragging;
    clear();
    if (moved !== null) void tabs.move(moved, group, null);
  }

  function dropLoose(event: DragEvent) {
    event.preventDefault();
    const moved = dragging;
    clear();
    // Dropped on the bar itself: out of every group.
    if (moved !== null) void tabs.move(moved, null, null);
  }
</script>

<!--
  Dragging a tab is how it joins a group, leaves one, or changes place.

  Three kinds of target, and the whole move is one call: dropping on a tab lands in front of it
  and in its group; dropping on a group's band or its name joins that group; dropping on the
  bar itself takes the tab out of every group.
-->
{#snippet chip(tab: Tab)}
  <div
    class="tab"
    class:active={tabs.session.active === tab.id}
    class:missing={tab.missing}
    class:dragging={dragging === tab.id}
    class:before={insertBefore === tab.id}
    draggable="true"
    role="presentation"
    ondragstart={(e) => start(e, tab.id)}
    ondragend={clear}
    ondragover={(e) => overTab(e, tab.id)}
    ondragleave={() => leaveTab(tab.id)}
    ondrop={(e) => dropOnTab(e, tab)}
  >
    <button class="pick" onclick={() => tabs.activate(tab.id)} title={tab.path}>
      {title(tab)}
    </button>
    <button class="shut" onclick={() => tabs.close(tab.id)} title="Close">×</button>
  </div>
{/snippet}

<nav
  class="bar"
  class:loose={dragging !== null && overBar}
  role="presentation"
  ondragover={overLoose}
  ondragleave={() => (overBar = false)}
  ondrop={dropLoose}
>
  {#each tabs.bands as band (band.group?.id ?? `loose-${band.tabs[0]?.id}`)}
    {#if band.group}
      {@const group = band.group}
      <div
        class="band"
        class:target={joining === group.id}
        style:--band="{bandColour(group.colour)}"
        role="presentation"
        ondragover={(e) => overBand(e, group.id)}
        ondragleave={() => leaveBand(group.id)}
        ondrop={(e) => dropOnBand(e, group.id)}
      >
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
            {@render chip(tab)}
          {/each}
        {/if}
      </div>
    {:else}
      {#each band.tabs as tab (tab.id)}
        {@render chip(tab)}
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
  /* The band a drop would join, outlined rather than filled so the group's own colour still
     reads as its identity. */
  .band.target {
    outline: 2px solid var(--accent); outline-offset: -1px;
    border-radius: var(--radius-1) var(--radius-1) 0 0;
  }
  /* Dropping here takes the tab out of every group, which needs saying while it is happening. */
  .bar.loose { box-shadow: inset 0 -2px 0 var(--accent); }
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
  .tab.dragging { opacity: 0.4; }
  /* Where it would land, drawn as an insertion line down the tab's leading edge rather than a
     fill, so the tab under the pointer stays readable. */
  .tab.before::after {
    content: ''; position: absolute; inset: 2px auto 2px -2px; width: 2px;
    background: var(--accent); border-radius: 1px;
  }
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
