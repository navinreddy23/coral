<script lang="ts">
  import ChromeMark from './ChromeMark.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import TabMark, { TAB_ICONS } from './TabMark.svelte';
  import { TabsState, type GroupColour, type Tab, type TabGroup, type TabIcon } from '../state/tabs.svelte';

  const { tabs, onOpen, onCloseNew, newTab, onAsk }: {
    tabs: TabsState;
    onOpen: () => void;
    /**
     * True while the start page is showing.
     *
     * It is a tab as far as anyone looking at the bar is concerned, so it is the one marked
     * as current; leaving the repository behind it filled said the window was showing that
     * repository, which it was not.
     */
    newTab: boolean;
    /** Puts the start page away, when there is a repository to go back to. */
    onCloseNew: () => void;
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
      {
        kind: 'item',
        label: 'Change icon…',
        run: () => {
          pickAt = { left: event.clientX, top: event.clientY };
          picking = tab;
        },
      },
      { kind: 'separator' },
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

  /**
   * A group is a coloured band around a contiguous run of tabs, as Chrome draws them.
   *
   * `soft` asks for the tinted form, which is what fills the band; the full strength is for
   * its name chip and its edge, where the colour has to be unmistakable.
   */
  function bandColour(colour: string | undefined, soft = false): string {
    if (!colour) return 'transparent';
    return `var(--${colour.replace('lane', 'lane-')}${soft ? '-soft' : ''})`;
  }

  function title(tab: Tab): string {
    return TabsState.title(tab);
  }

  /** The picture a tab shows: what was chosen for it, or a branch. */
  function iconOf(tab: Tab): TabIcon {
    return tab.icon ?? 'branch';
  }

  /**
   * The colour that picture is drawn in, from the path rather than from a preference.
   *
   * A browser's tab strip is legible at a glance because every favicon is a different colour,
   * and a strip of identical grey marks would be worse than none. Hashing the path gives each
   * checkout a colour that is its own, is the same on every launch, and costs the user no
   * decision — including for the eight tabs somebody opens before they read this sentence.
   */
  function hueOf(tab: Tab): string {
    let h = 0;
    for (let i = 0; i < tab.path.length; i += 1) h = (h * 31 + tab.path.charCodeAt(i)) % 4096;
    return `var(--lane-${(h % 8) + 1})`;
  }

  /**
   * The icon picker.
   *
   * A grid of the pictures themselves rather than a list of their names: choosing one by
   * reading the word "Beaker" is a worse way to pick a picture than looking at it.
   */
  let picking = $state<Tab | null>(null);
  let pickAt = $state({ left: 0, top: 0 });

  function pickIcon(tab: Tab, icon: TabIcon | null) {
    picking = null;
    void tabs.setIcon(tab.id, icon);
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
    class:active={!newTab && tabs.session.active === tab.id}
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
      <span class="icon" style:--hue={hueOf(tab)}><TabMark kind={iconOf(tab)} /></span>
      <span class="name">{title(tab)}</span>
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
        style:--band={bandColour(group.colour)}
        style:--band-soft={bandColour(group.colour, true)}
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

  {#if newTab}
    <div class="tab active new">
      <button class="pick" onclick={onOpen} title="Pick a repository to open">
        <span class="name">New tab</span>
      </button>
      {#if tabs.session.tabs.length > 0}
        <button class="shut" onclick={onCloseNew} title="Close">×</button>
      {/if}
    </div>
  {/if}
  <button class="add" onclick={onOpen} title="Open a repository">+</button>
  {#if tabs.error}<span class="error">{tabs.error}</span>{/if}

  <button
    class="find"
    class:on={searching}
    onclick={(e) => (searching ? (searching = false) : openSearch(e))}
    title="Search open tabs"
    aria-expanded={searching}
  ><ChromeMark kind="chevron" size={15} /></button>
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
            <span class="icon" style:--hue={hueOf(tab)}><TabMark kind={iconOf(tab)} /></span>
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

<svelte:window onkeydown={(e) => (e.key === 'Escape' && picking ? (picking = null) : null)} />

{#if picking}
  {@const tab = picking}
  <div class="scrim" role="presentation" onclick={() => (picking = null)}></div>
  <div class="picker" style:left="{pickAt.left}px" style:top="{pickAt.top}px">
    <p class="heading">Icon for {title(tab)}</p>
    <div class="grid" style:--hue={hueOf(tab)}>
      {#each TAB_ICONS as choice (choice.id)}
        <button
          class="cell"
          class:on={iconOf(tab) === choice.id}
          title={choice.label}
          aria-label={choice.label}
          onclick={() => pickIcon(tab, choice.id)}
        >
          <TabMark kind={choice.id} size={16} />
        </button>
      {/each}
    </div>
    <button class="reset" onclick={() => pickIcon(tab, null)}>Use the default</button>
  </div>
{/if}

{#if menu}
  <Menu x={menu.x} y={menu.y} items={menu.items} onClose={() => (menu = null)} />
{/if}

<style>
  /*
   * The tabs, standing on the title strip.
   *
   * The strip is the window's title bar and paints the ground and the line along its bottom;
   * this is the stretch of it the tabs occupy, and it takes whatever the rest of the strip
   * leaves. Empty space in it moves the window, which the strip itself handles, and which is
   * the whole reason the tabs are up here rather than on a row of their own.
   */
  .bar {
    /* The face the current tab wears, which is the toolbar's own: the two are meant to read as
       one surface stepping up out of the strip, and a tab painted any other colour is a tab
       sitting on the toolbar rather than joined to it. */
    --tab-face: var(--bg-1);
    flex: 1 1 auto; min-width: 0; align-self: stretch;
    display: flex; align-items: flex-end; gap: 0;
    padding: var(--space-2) 0 0;
    overflow-x: auto;
    /* A scrollbar here would be drawn inside the title bar and take a third of its height. The
       overflow still scrolls, by wheel and by the search panel beside it. */
    scrollbar-width: none;
  }
  /* A group is a tinted tray the tabs sit in, with its name on a chip at the leading edge. */
  .band {
    display: flex; align-items: flex-end; gap: 0;
    padding: 3px 4px 0;
    background: var(--band-soft);
    border-radius: 10px 10px 0 0;
  }
  /* The band a drop would join, outlined rather than filled so the group's own colour still
     reads as its identity. */
  .band.target { outline: 2px solid var(--accent); outline-offset: -1px; }
  /* Dropping here takes the tab out of every group, which needs saying while it is happening. */
  .bar.loose { box-shadow: inset 0 -3px 0 var(--accent); }
  .group {
    font: inherit; font-size: 11px; font-weight: 600; cursor: pointer; white-space: nowrap;
    align-self: center; margin: 0 var(--space-1) 4px var(--space-1);
    padding: 2px var(--space-2); border: 0; border-radius: 999px;
    background: var(--band); color: var(--bg-0);
  }
  /*
   * The count on a collapsed group, drawn in the chip's own two colours the other way round.
   * Any other pair is a guess that has to hold for eight band colours in two themes; this one
   * contrasts exactly as well as the name beside it does, by construction.
   */
  .group .tally {
    margin-left: var(--space-1); padding: 0 5px; border-radius: 999px;
    background: var(--bg-0); color: var(--band); font-size: 10px; font-weight: 700;
  }

  .tab {
    display: flex; align-items: center; position: relative;
    margin: 0 3px; min-height: 32px;
    border-radius: 10px 10px 0 0; background: transparent;
  }
  /*
   * A hairline between neighbours, the way a browser separates tabs that carry no fill of
   * their own. It goes wherever a tab is filled — its own hover, or the one before it — since
   * two edges meeting at a fill is already a boundary and the line only muddies it.
   */
  .tab:not(.active) + .tab:not(.active)::before {
    content: ''; position: absolute; left: -2px; top: 8px; bottom: 8px; width: 1px;
    background: var(--border-strong); opacity: 0.55;
  }
  .tab:not(.active):hover::before,
  .tab:hover + .tab:not(.active)::before,
  .tab.active + .tab:not(.active)::before { opacity: 0; }
  /*
   * The active tab is a step out of the strip and into the panel below it.
   *
   * It carries the panel's own fill and no bottom edge, so the two read as one surface, and
   * the two ears flare its base outward to meet the strip — which is the shape that makes a
   * browser's current tab findable without reading a word of it. The accent line along the
   * top is the second cue, for a strip where every tab is the same shade of white.
   */
  .tab.active {
    background: var(--tab-face); z-index: 2;
    box-shadow: inset 0 2px 0 var(--accent);
  }
  .tab.active .pick { color: var(--fg-0); font-weight: 600; }
  .tab.active::before, .tab.active::after {
    content: ''; position: absolute; bottom: 0; width: 10px; height: 10px;
    opacity: 1; pointer-events: none;
  }
  .tab.active::before {
    left: -10px;
    background: radial-gradient(circle at 0 0, transparent 10px, var(--tab-face) 10.5px);
  }
  .tab.active::after {
    right: -10px;
    background: radial-gradient(circle at 100% 0, transparent 10px, var(--tab-face) 10.5px);
  }
  .tab:hover:not(.active) { background: var(--bg-3); }
  .tab.dragging { opacity: 0.4; }
  /* Where it would land, drawn as an insertion line down the tab's leading edge rather than a
     fill, so the tab under the pointer stays readable. */
  .tab.before { box-shadow: inset 2px 0 0 var(--accent); }
  .tab.missing .name { text-decoration: line-through; color: var(--fg-2); }
  .tab.missing .icon { color: var(--fg-2); }

  .pick {
    display: flex; align-items: center; gap: var(--space-2);
    font: inherit; font-size: 12px; cursor: pointer; white-space: nowrap;
    max-width: 13em; min-width: 0;
    align-self: stretch; padding: 0 var(--space-1) 0 var(--space-3);
    background: transparent; border: 0; color: var(--fg-1);
  }
  .pick .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  /* The picture, in the colour that repository always gets. It keeps its colour on the tab
     that is not current: a strip of grey marks is no easier to read than no marks at all. */
  .icon { display: flex; flex: 0 0 auto; color: var(--hue, var(--fg-2)); }
  /* The close button appears on the tab being pointed at, and on the active one always: a row
     of crosses is noise, and a tab with no visible way to close it is a trap. */
  .shut {
    font: inherit; font-size: 13px; line-height: 1; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    width: 18px; height: 18px; margin-right: var(--space-2);
    padding: 0; border-radius: 50%;
    background: transparent; border: 0; color: var(--fg-2);
    visibility: hidden;
  }
  .tab:hover .shut, .tab.active .shut { visibility: visible; }
  .shut:hover { color: var(--danger); background: var(--danger-soft); }

  .add {
    font: inherit; font-size: 16px; line-height: 1; cursor: pointer; align-self: center;
    display: flex; align-items: center; justify-content: center;
    width: 26px; height: 26px; margin: 0 var(--space-1) 3px;
    padding: 0; background: transparent; border: 0; border-radius: 50%; color: var(--fg-2);
  }
  .add:hover { color: var(--fg-0); background: var(--bg-3); }
  .error { align-self: center; color: var(--danger); font-size: 11px; }

  /* Pinned to the trailing edge so it stays reachable however far the bar has scrolled — which
     is exactly the case it exists for. */
  /*
   * Beside the tabs it searches rather than at the far end of the strip, and pinned to the
   * right edge only once the bar has scrolled — which is the case it exists for.
   */
  .find {
    position: sticky; right: 0; flex: 0 0 auto; align-self: center;
    display: flex; align-items: center; justify-content: center; cursor: pointer;
    width: 26px; height: 26px; margin-bottom: 3px;
    padding: 0; border-radius: 50%;
    background: transparent; border: 0; color: var(--fg-2);
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
  .heading .tally {
    margin-left: auto; background: var(--bg-2); border-radius: 999px; padding: 0 6px;
    font-size: 10px;
  }
  .finder ul { list-style: none; margin: 0; padding: 0; }
  .finder li { display: flex; align-items: stretch; }
  .hit {
    display: flex; align-items: center; gap: var(--space-2);
    flex: 1; min-width: 0; text-align: left; font: inherit; font-size: 12px;
    padding: var(--space-2); border-radius: var(--radius-1); cursor: pointer;
    background: var(--bg-2); border: 0; color: var(--fg-0);
  }
  .hit:hover { background: var(--bg-2); }
  .hit.active { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line); }
  .what { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .what .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
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
    padding: 0 var(--space-2); background: var(--bg-2); border: 0; color: var(--fg-2);
  }
  .drop:hover { color: var(--danger); }
  .none { padding: var(--space-3); color: var(--fg-2); font-size: 12px; }

  /* The icon picker, hung where the menu was rather than under the tab: the menu is where the
     click that asked for it happened, and the bar may have scrolled since. */
  .picker {
    position: fixed; z-index: 41; width: 15em;
    padding: var(--space-2);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-2);
    box-shadow: 0 8px 24px rgb(0 0 0 / 18%);
  }
  .grid {
    display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--space-1);
    margin-bottom: var(--space-2);
  }
  .cell {
    display: flex; align-items: center; justify-content: center;
    aspect-ratio: 1; cursor: pointer; padding: 0;
    background: var(--bg-1); border: 1px solid transparent; border-radius: var(--radius-1);
    color: var(--hue, var(--fg-1));
  }
  .cell:hover { background: var(--bg-2); }
  .cell.on { border-color: var(--accent); background: var(--accent-soft); }
  .reset {
    width: 100%; font: inherit; font-size: 11px; cursor: pointer;
    padding: var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-1); border: 1px solid var(--border); color: var(--fg-1);
  }
  .reset:hover { background: var(--bg-2); color: var(--fg-0); }
</style>
