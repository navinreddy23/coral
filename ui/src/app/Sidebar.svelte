<script lang="ts">
  import { byVersionDescending } from './version';
  import type { PullRequest } from '../ipc/commands';
  import type { Remote, Submodule } from '../ipc/types';
  import type { IconName } from './icon';
  import type { PlacedRef, RefGroups } from '../state/refs.svelte';
  import type { PlacedStash } from '../ipc/stash';
  import HostMark, { hostOf } from './HostMark.svelte';
  import Icon from './Icon.svelte';
  import { elideRef } from './path';

  const {
    groups,
    head,
    detachedHead,
    stashes,
    submodules,
    remotes,
    openSubmodule,
    pullRequests,
    pullRequestLabel,
    onSelect,
    onOpenSubmodule,
    onDropRef,
    onOpenPullRequest,
    onRemoteMenu,
    onRefMenu,
    onStashMenu,
    onInitAllSubmodules,
    onSubmoduleMenu,
    collapsed,
    onCollapse,
    focusFilter,
    reveal,
    scope,
    onToggleHidden,
    onLeaveSolo,
    onShowEverything,
    onSelectRef,
  }: {
    groups: RefGroups;
    head: string | null;
    /**
     * Where HEAD is when it is on no branch, so the list has something to say about it.
     *
     * Checking a commit out detaches HEAD at it, and until now the panel showed nothing at
     * all: the branch list was of branches HEAD was not on, and where it actually was could
     * only be found by looking down the graph for the pill.
     */
    detachedHead: { oid: string; row: number | null } | null;
    /** The stash stack. Listed on its own, because `refs/stash` is only ever the top of it. */
    stashes: PlacedStash[];
    submodules: Submodule[];
    /** The configured remotes, so each can carry its own name and menu. */
    remotes: Remote[];
    /** The submodule currently open in this tab, marked in the list. */
    openSubmodule: string | null;
    /** Empty when the remote is not a recognised host, or no token is stored for it. */
    pullRequests: PullRequest[];
    /** What the host calls them: GitHub says pull, GitLab says merge. */
    pullRequestLabel: string;
    onSelect: (row: number) => void;
    /** What can be done with one stash, asked for at the dots or by right-clicking. */
    onStashMenu: (event: MouseEvent, stash: PlacedStash) => void;
    /** The same for a branch or a tag: checkout, merge, rebase, delete. */
    onRefMenu: (event: MouseEvent, ref: PlacedRef) => void;
    /** Shows a submodule inside this tab. */
    onOpenSubmodule: (path: string) => void;
    /** `source` was dragged onto `target`; the shell decides what that means. */
    onDropRef: (source: string, target: string) => void;
    onOpenPullRequest: (pr: PullRequest) => void;
    /** Right-click on a remote, or on the section itself when `remote` is null. */
    onRemoteMenu: (event: MouseEvent, remote: string | null) => void;
    /** Fetches a working copy for every submodule that has none. */
    onInitAllSubmodules: () => void;
    /** The dots, or a right-click, on one submodule. */
    onSubmoduleMenu: (event: MouseEvent, submodule: Submodule) => void;
    /**
     * Which sections are closed, by key. Held by the window rather than here, so a panel
     * closed on purpose is still closed after a restart.
     */
    collapsed: Record<string, boolean>;
    onCollapse: (section: string, closed: boolean) => void;
    /**
     * Bumped by the window to put the caret in the filter box.
     *
     * A counter rather than a flag: pressing the key twice has to work twice, and a flag set
     * to true while already true is not a change for an effect to see.
     */
    focusFilter: number;
    /**
     * A section the window wants brought into view, and a counter saying how often.
     *
     * The rail asks for this: its icons open the panel at one section, and on a repository
     * with a long branch list the section asked for is often below the fold. A counter for
     * the same reason `focusFilter` is one — asking twice for the same section has to work
     * twice.
     */
    reveal: { key: string; tick: number };
    /**
     * Which refs the graph is walked from, by full name.
     *
     * Given rather than read here, because the panel renders and the shell decides: soloing is
     * a change to what the engine walks, not to how this list is drawn.
     */
    scope: { solo: string | null; hidden: string[] };
    /** Takes one ref out of the walk, or puts it back. */
    onToggleHidden: (ref: PlacedRef) => void;
    /**
     * Leaves solo, and only solo.
     *
     * Whatever was hidden stays hidden: hiding is a standing preference somebody set earlier,
     * and giving it back unasked puts a branch they deliberately got rid of back in the graph
     * with nothing to say what did it.
     */
    onLeaveSolo: () => void;
    /** Unhides everything, from the button on the hidden-count banner. */
    onShowEverything: () => void;
    /**
     * A row was clicked. Separate from `onSelect` because a ref outside the current walk has no
     * row to go to, and the answer for it is to widen the view rather than to do nothing.
     */
    onSelectRef: (ref: PlacedRef) => void;
  } = $props();

  /** The name the scope is keyed by. Full, because `main` and `origin/main` are two tips. */
  const soloed = $derived(scope.solo);
  function isHidden(name: string): boolean {
    return scope.hidden.includes(name);
  }
  /** Whether the graph on screen was walked from this ref. */
  function walked(name: string): boolean {
    return soloed === null ? !isHidden(name) : soloed === name;
  }

  /** Ref being dragged, and the one under the pointer, so both can be marked. */
  let dragging = $state<string | null>(null);
  let over = $state<string | null>(null);

  function startDrag(event: DragEvent, short: string) {
    dragging = short;
    // A plain-text payload as well as the local state, so a drop onto another application
    // gets the branch name rather than nothing.
    event.dataTransfer?.setData('text/plain', short);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  }

  function dragOver(event: DragEvent, short: string) {
    if (dragging === null || dragging === short) return;
    // Preventing the default is what marks this a valid drop target.
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    over = short;
  }

  function drop(event: DragEvent, short: string) {
    event.preventDefault();
    const source = dragging;
    dragging = null;
    over = null;
    if (source !== null && source !== short) onDropRef(source, short);
  }

  let filter = $state('');
  let filterField = $state<HTMLInputElement | null>(null);
  // Only when asked, never on mount: an effect that reads the counter runs once as the panel
  // appears, and the window would open with the caret in the filter box, so the arrow keys
  // typed at the graph went into a text field instead.
  let focusedAt = 0;
  $effect(() => {
    if (focusFilter === focusedAt) return;
    focusedAt = focusFilter;
    filterField?.focus();
    filterField?.select();
  });

  let panel = $state<HTMLElement | null>(null);
  let revealedAt = 0;
  $effect(() => {
    if (reveal.tick === revealedAt) return;
    revealedAt = reveal.tick;
    const at = panel?.querySelector(`section[data-section="${reveal.key}"]`);
    at?.scrollIntoView?.({ block: 'nearest' });
  });
  /**
   * Sections rendered in full, by key.
   *
   * A capped list keeps the DOM small — a repository can carry tens of thousands of tags, and
   * the kernel carries 944 — but the cap has to be liftable, or the refs past it cannot be
   * reached at all.
   */
  let showingAll = $state<Record<string, boolean>>({});
  const CAP = 200;

  function shown(refs: PlacedRef[]): PlacedRef[] {
    const q = filter.trim().toLowerCase();
    return q ? refs.filter((r) => r.short.toLowerCase().includes(q)) : refs;
  }

  const matchingSubmodules = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return q ? submodules.filter((s) => s.path.toLowerCase().includes(q)) : submodules;
  });

  /**
   * Remote branches under the remote they belong to.
   *
   * A tracking branch is named `<remote>/<branch>`, so a flat list repeats the remote's name on
   * every row and still never says what that remote actually is. Grouping puts the remote where
   * its URL and its menu can hang off it, which is what the reference does.
   */
  const byRemote = $derived.by(() => {
    const out = new Map<string, PlacedRef[]>();
    for (const name of remotes.map((r) => r.name)) out.set(name, []);
    for (const ref of shown(groups.remote)) {
      const remote = ref.short.split('/')[0] ?? '';
      const list = out.get(remote);
      if (list) list.push(ref);
      else out.set(remote, [ref]);
    }
    return out;
  });

  /**
   * What a row says on hover.
   *
   * A ref with no row used to say "not in the loaded graph", which is true and, once a branch
   * is soloed, is what almost every row in the panel would say — a panel that reads as broken
   * rather than as narrowed. When the user narrowed it themselves, it says so instead.
   */
  function refTitle(r: PlacedRef, outside: boolean, hidden: boolean): string {
    if (hidden) return `${r.name}\nhidden from the graph`;
    if (outside && soloed !== null) return `${r.name}\noutside the branch being soloed`;
    if (r.row === null) return `${r.name}\nnot in the loaded graph`;
    return r.name;
  }

  /** The branch without its remote prefix, which is the only part that differs down the list. */
  function withoutRemote(short: string): string {
    const at = short.indexOf('/');
    return at < 0 ? short : short.slice(at + 1);
  }

  /** A full ref name as the panel writes it: `refs/remotes/origin/x` reads as `origin/x`. */
  function shortOf(name: string): string {
    return name
      .replace(/^refs\/heads\//u, '')
      .replace(/^refs\/remotes\//u, '')
      .replace(/^refs\/tags\//u, '');
  }

  function urlOf(name: string): string {
    return remotes.find((r) => r.name === name)?.fetchUrl ?? '';
  }

  /**
   * The section's own mark: the host every remote shares, or the cloud when they differ.
   *
   * A repository with a GitHub origin and a GitLab mirror belongs to neither, and claiming one
   * of them at the section level would be wrong half the time.
   */
  const sectionHost = $derived.by(() => {
    const kinds = new Set(remotes.map((r) => hostOf(r.fetchUrl)));
    const only = kinds.size === 1 ? [...kinds][0] : undefined;
    return only ?? 'other';
  });

  // Section order follows the reference's left panel: Local, Remote, Stashes, then Tags. The
  // remote section is rendered on its own because a remote is a thing with a menu, not a row.
  const above = $derived([
    { key: 'local', title: 'Local', icon: 'branch' as IconName, refs: shown(groups.local) },
  ]);
  // Newest first. The cap that keeps the DOM small takes the first two hundred rows, so the
  // order has to be right before it applies or the kernel's list would be capped at its oldest
  // tags and the release anyone wants would be behind a "Show all 944".
  const below = $derived([
    {
      key: 'tags',
      title: 'Tags',
      icon: 'tag' as IconName,
      refs: byVersionDescending(shown(groups.tags), (r) => r.short),
    },
  ]);

  /** The stack, filtered by the same box as everything else in this panel. */
  const stashRows = $derived(
    filter.trim() === ''
      ? stashes
      : stashes.filter((s) => `${s.name} ${s.message}`.toLowerCase().includes(filter.trim().toLowerCase())),
  );

  const total = $derived(
    groups.local.length + groups.remote.length + groups.tags.length + stashes.length,
  );
</script>

{#snippet refRow(r: PlacedRef, label: string, draggable: boolean)}
  {@const hidden = isHidden(r.name)}
  {@const outside = !walked(r.name)}
  <li>
    <div
      class="row"
      class:outside
      class:soloed={soloed === r.name}
      oncontextmenu={(e) => onRefMenu(e, r)}
      role="presentation"
    >
    <button
      class="ref"
      class:current={r.short === head}
      class:dragging={dragging === r.short}
      class:over={over === r.short}
      {draggable}
      ondragstart={(e) => startDrag(e, r.short)}
      ondragend={() => {
        dragging = null;
        over = null;
      }}
      ondragover={(e) => dragOver(e, r.short)}
      ondragleave={() => (over = over === r.short ? null : over)}
      ondrop={(e) => drop(e, r.short)}
      onclick={() => onSelectRef(r)}
      title={refTitle(r, outside, hidden)}
    >
      <span class="tick">
        {#if r.short === head}<Icon name="check" size={12} />{/if}
      </span>
      <span class="text">{elideRef(label, 28)}</span>
      {#if r.ahead > 0 || r.behind > 0}
        <span class="track">{r.ahead}↑ {r.behind}↓</span>
      {/if}
    </button>
    <button
      class="eye"
      class:off={hidden}
      title={hidden ? `Show ${r.short} in the graph` : `Hide ${r.short} from the graph`}
      onclick={(e) => {
        e.stopPropagation();
        onToggleHidden(r);
      }}
    ><Icon name={hidden ? 'eyeOff' : 'eye'} size={12} /></button>
    <button
      class="dots"
      title="What can be done with {r.short}"
      onclick={(e) => onRefMenu(e, r)}
    ><Icon name="more" size={13} /></button>
    </div>
  </li>
{/snippet}

<aside bind:this={panel}>
  <!--
    Solo is a mode, and a mode the graph does not otherwise announce: the window would simply
    be missing most of its commits, with nothing anywhere saying why or how to get them back.
    Hiding needs no banner — the struck eye stays on the row it belongs to — but soloing takes
    every other branch off the screen at once, so it says so where the branches were.
  -->
  {#if soloed !== null}
    <div class="solo-banner">
      <span class="tag">SOLO</span>
      <span class="who" title={soloed}>{elideRef(shortOf(soloed), 20)}</span>
      <button class="leave" onclick={onLeaveSolo}>Leave</button>
    </div>
  {:else if scope.hidden.length > 0}
    <div class="solo-banner quiet">
      <span class="who">{scope.hidden.length} hidden</span>
      <button class="leave" onclick={onShowEverything}>Show all</button>
    </div>
  {/if}
  <!--
    How much of the repository the graph is drawn from. Not while a branch is soloed: the
    banner above already says the walk is scoped to one, and this counts every ref in the
    repository — so the two lines were answering the same question differently, on the one
    screen where they appear together.
  -->
  {#if soloed === null}
    <p class="viewing" title="Branches, tags and stashes the graph is drawn from">
      Viewing <strong>{total}</strong> refs
    </p>
  {/if}
  <!-- The magnifier is inside the field rather than beside it: a box labelled only by its
       placeholder loses that label the moment somebody types in it. -->
  <div class="search">
    <span class="lens"><Icon name="search" size={13} /></span>
    <input
      class="filter"
      placeholder="Filter branches, tags and stashes"
      bind:this={filterField}
      bind:value={filter}
    />
    {#if filter !== ''}
      <button class="clear" title="Clear the filter" onclick={() => (filter = '')}>
        <Icon name="close" size={12} />
      </button>
    {/if}
  </div>

  {#each above as section (section.key)}
    <section data-section="{section.key}">
      <button class="head" onclick={() => onCollapse(section.key, !collapsed[section.key])}>
        <span class="caret">
          <Icon name={collapsed[section.key] ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        <span class="icon"><Icon name={section.icon} size={13} /></span>
        {section.title}
        <span class="count">
          {section.refs.length + (section.key === 'local' && detachedHead !== null ? 1 : 0)}
        </span>
      </button>
      {#if !collapsed[section.key]}
        <ul>
          {#if section.key === 'local' && detachedHead !== null}
            <li>
              <div class="row">
                <button
                  class="ref current"
                  disabled={detachedHead.row === null}
                  onclick={() => detachedHead?.row !== null && onSelect(detachedHead.row)}
                  title={detachedHead.row === null
                    ? `HEAD is detached at ${detachedHead.oid}\nnot in the loaded graph`
                    : `HEAD is detached at ${detachedHead.oid}`}
                >
                  <span class="tick"><Icon name="check" size={12} /></span>
                  <span class="text">HEAD</span>
                  <span class="track mono">{detachedHead.oid.slice(0, 8)}</span>
                </button>
              </div>
            </li>
          {/if}
          {#each showingAll[section.key] ? section.refs : section.refs.slice(0, CAP) as r (r.name)}
            {@render refRow(r, r.short, section.key === 'local')}
          {/each}
          {#if section.refs.length > CAP && !showingAll[section.key]}
            <li>
              <button class="more" onclick={() => (showingAll[section.key] = true)}>
                Show all {section.refs.length}
              </button>
            </li>
          {/if}
        </ul>
      {/if}
    </section>
  {/each}

  <!--
    Remotes carry their own row, because a remote is a thing with a URL that can be edited,
    renamed and removed — not merely a prefix on a branch name.
  -->
  <section data-section="remote">
    <div class="row head-row" oncontextmenu={(e) => onRemoteMenu(e, null)} role="presentation">
      <button
        class="head"
        onclick={() => onCollapse('remote', !collapsed['remote'])}
        title="Remotes"
      >
        <span class="caret">
          <Icon name={collapsed['remote'] ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        <span class="icon"><HostMark kind={sectionHost} /></span>
        Remote
        <span class="count">{shown(groups.remote).length}</span>
      </button>
      <button
        class="dots"
        title="Add or manage remotes"
        onclick={(e) => onRemoteMenu(e, null)}
      ><Icon name="more" size={13} /></button>
    </div>
    {#if !collapsed['remote']}
      {#if byRemote.size === 0}
        <p class="none">
          No remotes. <button class="link" onclick={(e) => onRemoteMenu(e, null)}>Add one</button>
        </p>
      {/if}
      {#each [...byRemote] as [name, refs] (name)}
        {@const key = `remote:${name}`}
        <div class="row" oncontextmenu={(e) => onRemoteMenu(e, name)} role="presentation">
          <button
            class="head remote"
            onclick={() => onCollapse(key, !collapsed[key])}
            title={`${name}\n${urlOf(name)}`}
          >
            <span class="caret">
          <Icon name={collapsed[key] ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
            <span class="icon">
              <HostMark kind={urlOf(name) === '' ? 'other' : hostOf(urlOf(name))} />
            </span>
            <span class="text">{name}</span>
            <span class="count">{refs.length}</span>
          </button>
          <button
            class="dots"
            title="What can be done with {name}"
            onclick={(e) => onRemoteMenu(e, name)}
          ><Icon name="more" size={13} /></button>
        </div>
        {#if !collapsed[key]}
          <ul class="nested">
            {#each showingAll[key] ? refs : refs.slice(0, CAP) as r (r.name)}
              {@render refRow(r, withoutRemote(r.short), true)}
            {/each}
            {#if refs.length > CAP && !showingAll[key]}
              <li>
                <button class="more" onclick={() => (showingAll[key] = true)}>
                  Show all {refs.length}
                </button>
              </li>
            {/if}
          </ul>
        {/if}
      {/each}
    {/if}
  </section>

  <!--
    Stashes are listed from the stack rather than from the refs, and each row carries its own
    menu: applying one, popping it and dropping it are three different things and only one of
    them can be undone.
  -->
  <section data-section="stashes">
    <button class="head" onclick={() => onCollapse('stashes', !collapsed['stashes'])}>
      <span class="caret">
          <Icon name={collapsed['stashes'] ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
      <span class="icon"><Icon name="stash" size={12} /></span>
      Stashes
      <span class="count">{stashRows.length}</span>
    </button>
    {#if !collapsed['stashes']}
      {#if stashRows.length === 0}
        <p class="none">Nothing stashed.</p>
      {/if}
      <ul>
        {#each stashRows as stash (stash.oid)}
          <li>
            <div class="row" oncontextmenu={(e) => onStashMenu(e, stash)} role="presentation">
              <button
                class="ref"
                disabled={stash.row === null}
                onclick={() => stash.row !== null && onSelect(stash.row)}
                title={stash.row === null
                  ? `${stash.name}\n${stash.message}\nnot in the loaded graph`
                  : `${stash.name}\n${stash.message}`}
              >
                <!-- The same gutter every other row keeps, so the four sections line up. -->
                <span class="tick"></span>
                <span class="text">{elideRef(stash.name, 24)}</span>
              </button>
              <button
                class="dots"
                title="What can be done with {stash.name}"
                onclick={(e) => onStashMenu(e, stash)}
              ><Icon name="more" size={13} /></button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#each below as section (section.key)}
    <section data-section="{section.key}">
      <button class="head" onclick={() => onCollapse(section.key, !collapsed[section.key])}>
        <span class="caret">
          <Icon name={collapsed[section.key] ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        <span class="icon"><Icon name={section.icon} size={13} /></span>
        {section.title}
        <span class="count">{section.refs.length}</span>
      </button>
      {#if !collapsed[section.key]}
        <ul>
          {#each showingAll[section.key] ? section.refs : section.refs.slice(0, CAP) as r (r.name)}
            {@render refRow(r, r.short, section.key === 'local')}
          {/each}
          {#if section.refs.length > CAP && !showingAll[section.key]}
            <li>
              <button class="more" onclick={() => (showingAll[section.key] = true)}>
                Show all {section.refs.length}
              </button>
            </li>
          {/if}
        </ul>
      {/if}
    </section>
  {/each}

  {#if pullRequests.length > 0}
    <section data-section="prs">
      <button class="head" onclick={() => onCollapse('prs', !collapsed['prs'])}>
        <span class="caret">
          <Icon name={collapsed['prs'] ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        <span class="icon"><Icon name="request" size={13} /></span>
        {pullRequestLabel}
        <span class="count">{pullRequests.length}</span>
      </button>
      {#if !collapsed['prs']}
        <ul>
          {#each pullRequests as pr (pr.number)}
            <li>
              <button
                class="ref pr"
                onclick={() => onOpenPullRequest(pr)}
                title={`${pr.title}\n${pr.author}: ${pr.sourceBranch} → ${pr.targetBranch}\n${pr.webUrl}`}
              >
                <span class="state {pr.state}">{pr.state[0]?.toUpperCase()}</span>
                <span class="num">#{pr.number}</span>
                <span class="title">{pr.title}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

  {#if submodules.length > 0}
    <section data-section="submodules">
      <button class="head" onclick={() => onCollapse('submodules', !collapsed['submodules'])}>
        <span class="caret">
          <Icon name={collapsed['submodules'] ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        <span class="icon"><Icon name="folder" size={13} /></span>
        Submodules
        <span class="count">{matchingSubmodules.length}</span>
      </button>
      {#if !collapsed['submodules'] && submodules.some((s) => !s.initialised)}
        <button class="more" onclick={onInitAllSubmodules}>
          Fetch every missing working copy
        </button>
      {/if}
      {#if !collapsed['submodules']}
        <ul>
          {#each matchingSubmodules as sub (sub.path)}
            <li oncontextmenu={(e) => onSubmoduleMenu(e, sub)}>
              <!--
                A submodule row is not a link. Clicking one used to open it, which is a choice
                nobody asked for on a row that also has to offer editing, updating and deleting
                — so every one of those is behind the same menu, reachable by the dots or by a
                right-click.
              -->
              <div class="row" class:current={sub.path === openSubmodule}>
                <span class="ref static" class:absent={!sub.initialised}
                  title={sub.initialised
                    ? `${sub.url || sub.name}\nRight-click, or use the dots, for what can be done with it`
                    : `${sub.url || sub.name}\nNo working copy yet`}
                >
                  <span class="tick"><Icon name={sub.initialised ? 'check' : 'arrowDown'} size={12} /></span>
                  <span class="text">{sub.path}</span>
                </span>
                <button
                  class="dots"
                  title="What can be done with this submodule"
                  onclick={(e) => onSubmoduleMenu(e, sub)}
                ><Icon name="more" size={13} /></button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
</aside>

<style>
  aside {
    width: var(--sidebar-w, 240px); flex: 0 0 auto; overflow-y: auto;
    border-right: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-2);
  }
  /* A count, not a heading: it says how much of the repository the graph is drawn from, and
     it used to shout that in 11px uppercase above everything else in the panel. */
  .viewing {
    margin: var(--space-1) var(--space-1) var(--space-2);
    font-size: var(--text-sm); color: var(--fg-2);
  }
  .viewing strong {
    color: var(--fg-1); font-weight: 600; font-variant-numeric: tabular-nums;
  }
  .icon {
    width: 1.1em; color: var(--fg-2); flex: 0 0 auto;
    display: inline-flex; align-items: center; justify-content: center;
  }
  .search {
    display: flex; align-items: center; gap: var(--space-1);
    margin-bottom: var(--space-2); padding: 0 var(--space-2);
    border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0);
    transition: border-color var(--fast) var(--ease);
  }
  .search:focus-within { border-color: var(--accent); }
  .lens { display: flex; color: var(--fg-2); flex: 0 0 auto; }
  .filter {
    flex: 1; min-width: 0; font: inherit; font-size: var(--text-base);
    padding: 4px 0; border: 0; background: var(--bg-0); color: var(--fg-0);
  }
  .filter:focus { outline: none; }
  .filter::placeholder { color: var(--fg-2); }
  /* Appears only when there is something to clear, so the field is not carrying a control
     that would do nothing nine times out of ten. */
  .clear {
    flex: 0 0 auto; display: flex; padding: 2px; cursor: pointer;
    background: var(--bg-0); border: 0; border-radius: var(--radius-1); color: var(--fg-2);
  }
  .clear:hover { color: var(--fg-0); background: var(--bg-2); }

  section + section { border-top: 1px solid var(--border); }
  /*
   * Every row that carries text paints its own opaque background.
   *
   * WebKit antialiases text on a composited layer with subpixel precision only where it knows
   * what is behind it. `background: none` leaves it guessing, so it falls back to grayscale and
   * the row reads soft — which is why hovering one used to sharpen it: the hover background was
   * the only thing telling WebKit what the backdrop was. The whole page is composited, because
   * that is what makes the wheel scroll at all.
   */
  .head {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; font: inherit; font-size: var(--text-base); font-weight: 600;
    color: var(--fg-2);
    background: var(--bg-1); border: 0; padding: var(--space-1) var(--space-1); cursor: pointer;
    border-radius: var(--radius-1);
    transition: color var(--fast) var(--ease), background var(--fast) var(--ease);
  }
  .head:hover { color: var(--fg-0); background: var(--bg-2); }
  /* A remote is a row inside the section, so it is indented and set in the ordinary face: it
     names a thing, where the section above it names a kind. */
  .head.remote {
    padding-left: var(--space-3); font-weight: 500; color: var(--fg-1);
  }
  .head.remote:hover { color: var(--fg-0); background: var(--bg-2); }
  /* A pill rather than a bare number: a count is a different kind of thing from the name
     beside it, and at this size only shape says so. */
  .count {
    margin-left: auto; color: var(--fg-2); font-size: var(--text-xs); font-weight: 600;
    background: var(--bg-2); border-radius: var(--radius-pill); padding: 1px 6px; min-width: 18px;
    text-align: center; flex: 0 0 auto; font-variant-numeric: tabular-nums;
  }
  .head:hover .count { background: var(--bg-3); }
  .caret { display: flex; color: var(--fg-2); flex: 0 0 auto; }
  .icon { display: flex; color: var(--fg-2); flex: 0 0 auto; }
  ul { list-style: none; margin: 0 0 var(--space-2); padding: 0; }
  ul.nested { margin-left: var(--space-3); }
  .ref {
    display: flex; align-items: center; gap: var(--space-2);
    flex: 1; min-width: 0; text-align: left; font: inherit; font-size: var(--text-base);
    padding: 3px var(--space-2) 3px var(--space-1);
    background: var(--bg-1); border: 0; border-radius: var(--radius-1); cursor: pointer;
    color: var(--fg-1); overflow: hidden;
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  /*
   * The gutter the tick sits in, present on every row whether it is ticked or not.
   *
   * Without it the branch you are on started fourteen pixels further right than the ones you
   * are not, so a column of names was ragged and the one fact this panel exists to show was
   * the reason for it.
   */
  .tick {
    flex: 0 0 auto; width: 14px; display: flex; justify-content: center;
    color: var(--accent);
  }
  .text { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis;
          white-space: nowrap; }
  .ref:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  .ref:disabled { color: var(--fg-2); cursor: default; }
  /* Declared but not fetched. Dimmed, but still a live target: clicking it fetches one. */
  .ref.absent { color: var(--fg-2); }
  .ref.absent .tick { color: var(--fg-2); }
  /* A submodule or remote row is not one control: it names a thing, and everything that can be
     done with it lives behind the dots beside it. */
  .row { display: flex; align-items: center; border-radius: var(--radius-1); }
  /* The section heading keeps its own spacing; only the dots are added to it. */
  .row .head { flex: 1; min-width: 0; }
  .row.head-row:hover { background: none; }
  .row:hover { background: var(--bg-2); }
  .row.current { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line); }
  .ref.static { cursor: default; }
  .dots {
    flex: 0 0 auto; display: flex; font: inherit; line-height: 1; cursor: pointer;
    padding: 0 var(--space-2); background: var(--bg-1); border: 0; color: var(--fg-2);
    visibility: hidden;
  }
  .row:hover .dots, .row.current .dots { visibility: visible; }
  .dots:hover { color: var(--fg-0); }
  /*
   * The eye follows the dots, with one difference that matters: a hidden branch keeps it
   * showing. Revealed only on hover, the single thing on screen saying a branch is hidden
   * would be behind the pointer, and a list with three branches quietly missing from the graph
   * would look like a list with nothing wrong with it.
   */
  .eye {
    flex: 0 0 auto; display: flex; align-items: center; cursor: pointer;
    padding: 0 var(--space-1); background: var(--bg-1); border: 0; color: var(--fg-2);
    visibility: hidden;
  }
  .row:hover .eye, .eye.off { visibility: visible; }
  .eye:hover { color: var(--fg-0); }
  .eye.off { color: var(--fg-2); }
  /* Dimmed, not hidden: a branch left out of the walk is still a branch, and still has a menu. */
  .row.outside .ref, .row.outside .text { color: var(--fg-2); }
  /*
   * On the button, not on the row around it. Every surface carrying text paints its own opaque
   * background, so a tint on the row is painted over by the name sitting on it and all that
   * survives is a stub of colour past the end of the text.
   *
   * The bar and the weight, and deliberately not the tint the checked-out row carries. Soloing
   * the branch above or below the one you are on gave two tinted rows in the same colour with
   * no edge between them, which reads as one selection two rows tall. The banner is what says
   * solo is on; this only has to make the row findable underneath it.
   */
  .row.soloed .ref {
    color: var(--fg-0); font-weight: 600;
    box-shadow: inset 2px 0 0 var(--accent-line);
  }
  /*
   * The banner. It sits above the filter rather than below it because the filter is a thing
   * the user is doing now and the mode is the thing they set earlier and may have forgotten.
   */
  .solo-banner {
    display: flex; align-items: center; gap: var(--space-2);
    margin: 0 0 var(--space-2); padding: var(--space-1) var(--space-2);
    background: var(--accent-soft); border-radius: var(--radius-1);
    box-shadow: inset 2px 0 0 var(--accent-line);
  }
  .solo-banner.quiet { background: var(--bg-2); box-shadow: inset 2px 0 0 var(--border-strong); }
  .solo-banner .tag {
    flex: 0 0 auto; font-size: var(--text-mark); font-weight: 700; letter-spacing: 0.08em;
    color: var(--accent); background: var(--bg-0); border-radius: 3px;
    padding: 1px var(--space-1);
  }
  .solo-banner .who {
    flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    font-size: var(--text-sm); font-weight: 600; color: var(--fg-0);
  }
  .solo-banner .leave {
    flex: 0 0 auto; font: inherit; font-size: var(--text-xs); cursor: pointer;
    padding: 1px var(--space-2); border: 1px solid var(--border-strong);
    border-radius: 3px; background: var(--bg-0); color: var(--fg-1);
  }
  .solo-banner .leave:hover { color: var(--fg-0); border-color: var(--accent); }
  /*
   * The checked-out branch, marked by a bar down its leading edge as well as a tint. The tint
   * alone is easy to lose against a hover, and this is the one row in the panel that has to
   * be findable at a glance.
   */
  .ref.current {
    color: var(--fg-0); font-weight: 600; background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent-line);
  }
  .ref.dragging { opacity: 0.5; }
  .pr { gap: var(--space-1); }
  .num { flex: 0 0 auto; color: var(--fg-2); font-size: var(--text-sm); }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* Fourteen wide because that is the gutter every other row keeps for its tick: the circle
     stands in for it, so a proposal's number begins where a branch's name does. */
  .state {
    flex: 0 0 auto; width: 14px; height: 13px; margin-right: var(--space-1);
    line-height: 13px; text-align: center;
    border-radius: 50%; font-size: var(--text-mark); font-weight: 700; color: var(--bg-0);
  }
  .state.open { background: var(--ok); }
  .state.draft { background: var(--fg-2); }
  .state.merged { background: var(--accent); }
  .state.closed { background: var(--danger); }
  /* The drop target, outlined rather than filled so the branch name stays readable under it. */
  .ref.over { outline: 2px solid var(--accent); outline-offset: -2px; border-radius: 3px; }
  .tick { color: var(--accent); flex: 0 0 auto; }
  .track { margin-left: auto; font-size: var(--text-xs); color: var(--fg-2); flex: 0 0 auto; }
  .none { margin: 0 0 var(--space-2) var(--space-4); font-size: var(--text-sm); color: var(--fg-2); }
  .more, .link {
    display: block; text-align: left; cursor: pointer;
    padding: 3px var(--space-4); font-size: var(--text-sm); color: var(--accent);
    background: var(--bg-1); border: 0; font-family: inherit;
  }
  .link { display: inline; padding: 0; }
  .more { width: 100%; }
  .more:hover, .link:hover { text-decoration: underline; }
</style>
