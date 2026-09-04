<script lang="ts">
  import Menu, { type MenuItem } from './Menu.svelte';
  import { TabsState, type GroupColour, type Tab, type TabGroup } from '../state/tabs.svelte';

  const { tabs, onOpen, onAsk }: {
    tabs: TabsState;
    onOpen: () => void;
    /** Asks the user for a line of text. Returns null when they cancelled. */
    onAsk: (title: string, detail: string, initial: string) => Promise<string | null>;
  } = $props();

  /** The menu on screen, if any. */
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  /**
   * The tab search.
   *
   * The bar scrolls once there are more tabs than fit, and a scrolled bar is no way to find one
   * repository among twenty. This lists them all, filtered, with the group each belongs to.
   */
  let searching = $state(false);
  let query = $state('');
  let field = $state<HTMLInputElement | null>(null);

  const found = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (q === '') return tabs.session.tabs;
    // Matched on the whole path, not only the name: two checkouts of the same repository have
    // the same name and differ only in where they are.
    return tabs.session.tabs.filter((t) => t.path.toLowerCase().includes(q));
  });

  function groupOf(tab: Tab): TabGroup | null {
    return tabs.session.groups.find((g) => g.id === tab.group) ?? null;
  }

  /**
   * Where the panel hangs, measured from the button.
   *
   * Fixed rather than absolute: the bar scrolls horizontally, and a panel positioned inside it
   * would be clipped by that overflow — which is precisely the case the search exists for.
   */
  let anchor = $state({ right: 12, top: 80 });

  function openSearch(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    anchor = { right: Math.max(8, window.innerWidth - box.right), top: box.bottom + 4 };
    searching = true;
    query = '';
    // The field is created by this same change, so focusing it has to wait for the DOM.
    queueMicrotask(() => field?.focus());
  }

  function searchKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      searching = false;
      return;
    }
    if (event.key === 'Enter') {
      const first = found[0];
      if (first) {
        void tabs.activate(first.id);
        searching = false;
      }
    }
  }

  const COLOURS: { id: GroupColour; label: string }[] = [
    { id: 'lane1', label: 'Blue' },
    { id: 'lane2', label: 'Pink' },
    { id: 'lane3', label: 'Amber' },
    { id: 'lane4', label: 'Green' },
    { id: 'lane5', label: 'Violet' },
    { id: 'lane6', label: 'Orange' },
    { id: 'lane7', label: 'Teal' },
    { id: 'lane8', label: 'Olive' },
  ];

  async function newGroup(tab: Tab) {
    const name = await onAsk('New tab group', 'Tabs in a group sit together under one band.', '');
    if (name !== null && name.trim() !== '') await tabs.group(name.trim(), [tab.id]);
  }

  async function renameGroup(group: TabGroup) {
    const name = await onAsk('Rename group', '', group.name);
    if (name !== null && name.trim() !== '') await tabs.rename(group.id, name.trim());
  }

  /** What right-clicking a tab offers: where it lives, and closing it. */
  function tabMenu(event: MouseEvent, tab: Tab) {
    event.preventDefault();
    const others = tabs.session.groups.filter((g) => g.id !== tab.group);
    const items: MenuItem[] = [
      { kind: 'item', label: 'New tab group…', run: () => void newGroup(tab) },
    ];
    if (others.length > 0) {
      items.push({
        kind: 'submenu',
        label: 'Add to group',
        items: others.map((g) => ({
          kind: 'item' as const,
          label: g.name,
          run: () => void tabs.move(tab.id, g.id, null),
        })),
      });
    }
    if (tab.group !== null) {
      items.push({
        kind: 'item',
        label: 'Remove from group',
        run: () => void tabs.move(tab.id, null, null),
      });
    }
    items.push(
      { kind: 'separator' },
      { kind: 'item', label: 'Close tab', danger: true, run: () => void tabs.close(tab.id) },
    );
    menu = { x: event.clientX, y: event.clientY, items };
  }

  /** What right-clicking a group's name offers. */
  function groupMenu(event: MouseEvent, group: TabGroup) {
    event.preventDefault();
    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { kind: 'item', label: 'Rename group…', run: () => void renameGroup(group) },
        {
          kind: 'submenu',
          label: 'Colour',
          items: COLOURS.map((c) => ({
            kind: 'item' as const,
            label: c.label,
            hint: group.colour === c.id ? '✓' : undefined,
            run: () => void tabs.recolour(group.id, c.id),
          })),
        },
        {
          kind: 'item',
          label: group.collapsed ? 'Expand group' : 'Collapse group',
          run: () => void tabs.setCollapsed(group.id, !group.collapsed),
        },
        { kind: 'separator' },
        {
          kind: 'item',
          label: 'Ungroup, keeping the tabs',
          run: () => void tabs.dissolve(group.id),
        },
        {
          kind: 'item',
          label: 'Close every tab in the group',
          danger: true,
          run: () => void tabs.closeGroup(group.id),
        },
      ],
    };
  }

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
    oncontextmenu={(e) => tabMenu(e, tab)}
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
          oncontextmenu={(e) => groupMenu(e, group)}
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

  <button
    class="find"
    class:on={searching}
    onclick={(e) => (searching ? (searching = false) : openSearch(e))}
    title="Search open tabs"
    aria-expanded={searching}
  >⌄</button>
</nav>

{#if searching}
  <div class="scrim" role="presentation" onclick={() => (searching = false)}></div>
  <div class="finder" style:right="{anchor.right}px" style:top="{anchor.top}px">
    <input
      bind:this={field}
      bind:value={query}
      onkeydown={searchKey}
      placeholder="Search tabs"
      spellcheck="false"
    />
    <p class="heading">Open tabs<span class="tally">{found.length}</span></p>
    <ul>
      {#each found as tab (tab.id)}
        {@const group = groupOf(tab)}
        <li>
          <button
            class="hit"
            class:active={tabs.session.active === tab.id}
            onclick={() => {
              void tabs.activate(tab.id);
              searching = false;
            }}
            title={tab.path}
          >
            <span class="glyph" aria-hidden="true">⑂</span>
            <span class="what">
              <span class="name">{title(tab)}</span>
              <span class="where">{tab.path}</span>
            </span>
            {#if group}
              <span class="tag" style:--band={bandColour(group.colour)}>{group.name}</span>
            {/if}
          </button>
          <button class="drop" onclick={() => tabs.close(tab.id)} title="Close">×</button>
        </li>
      {/each}
      {#if found.length === 0}
        <li class="none">Nothing open matches that.</li>
      {/if}
    </ul>
  </div>
{/if}

{#if menu}
  <Menu x={menu.x} y={menu.y} items={menu.items} onClose={() => (menu = null)} />
{/if}

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

  /* Pinned to the trailing edge so it stays reachable however far the bar has scrolled — which
     is exactly the case it exists for. */
  .find {
    position: sticky; right: 0; margin-left: auto; align-self: center; flex: 0 0 auto;
    font: inherit; font-size: 13px; line-height: 1; cursor: pointer;
    padding: 3px var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .find:hover, .find.on { background: var(--bg-3); color: var(--fg-0); }

  .scrim { position: fixed; inset: 0; z-index: 40; }
  .finder {
    position: fixed; z-index: 41;
    width: min(26em, calc(100vw - 2 * var(--space-3)));
    max-height: 60vh; overflow-y: auto;
    padding: var(--space-2);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-2);
    box-shadow: 0 8px 24px rgb(0 0 0 / 18%);
  }
  .finder input {
    width: 100%; box-sizing: border-box; font: inherit; font-size: 12px;
    padding: var(--space-2); margin-bottom: var(--space-2);
    border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  .finder input:focus { border-color: var(--accent); }
  .heading {
    display: flex; align-items: center; gap: var(--space-2);
    margin: 0 0 var(--space-1) var(--space-1);
    font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em;
    color: var(--fg-2);
  }
  .tally {
    margin-left: auto; background: var(--bg-2); border-radius: 999px; padding: 0 6px;
    font-size: 10px;
  }
  .finder ul { list-style: none; margin: 0; padding: 0; }
  .finder li { display: flex; align-items: stretch; }
  .hit {
    display: flex; align-items: center; gap: var(--space-2);
    flex: 1; min-width: 0; text-align: left; font: inherit; font-size: 12px;
    padding: var(--space-2); border-radius: var(--radius-1); cursor: pointer;
    background: none; border: 0; color: var(--fg-0);
  }
  .hit:hover { background: var(--bg-2); }
  .hit.active { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line); }
  .glyph { flex: 0 0 auto; color: var(--fg-2); }
  .what { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .where {
    font-size: 10px; color: var(--fg-2);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /* The group's own colour, so a tab found here is placed as well as named. */
  .tag {
    flex: 0 0 auto; font-size: 10px; padding: 0 6px; border-radius: 999px;
    background: var(--band); color: var(--bg-0);
  }
  .drop {
    flex: 0 0 auto; font: inherit; font-size: 14px; line-height: 1; cursor: pointer;
    padding: 0 var(--space-2); background: none; border: 0; color: var(--fg-2);
  }
  .drop:hover { color: var(--danger); }
  .none { padding: var(--space-3); color: var(--fg-2); font-size: 12px; }
</style>
