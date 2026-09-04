<script lang="ts">
  import type { PullRequest } from '../ipc/commands';
  import type { Remote, Submodule } from '../ipc/types';
  import type { PlacedRef, RefGroups } from '../state/refs.svelte';
  import type { PlacedStash } from '../ipc/stash';
  import HostMark, { hostOf } from './HostMark.svelte';
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
  } = $props();

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

  /** The branch without its remote prefix, which is the only part that differs down the list. */
  function withoutRemote(short: string): string {
    const at = short.indexOf('/');
    return at < 0 ? short : short.slice(at + 1);
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
  const above = $derived([{ key: 'local', title: 'Local', icon: '🖿', refs: shown(groups.local) }]);
  const below = $derived([{ key: 'tags', title: 'Tags', icon: '🏷', refs: shown(groups.tags) }]);

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
  <li>
    <div class="row" oncontextmenu={(e) => onRefMenu(e, r)} role="presentation">
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
      disabled={r.row === null}
      onclick={() => r.row !== null && onSelect(r.row)}
      title={r.row === null ? `${r.name}\nnot in the loaded graph` : r.name}
    >
      {#if r.short === head}<span class="tick" aria-hidden="true">✓</span>{/if}
      <span class="text">{elideRef(label, 28)}</span>
      {#if r.ahead > 0 || r.behind > 0}
        <span class="track">{r.ahead}↑ {r.behind}↓</span>
      {/if}
    </button>
    <button
      class="dots"
      title="What can be done with {r.short}"
      onclick={(e) => onRefMenu(e, r)}
    >⋮</button>
    </div>
  </li>
{/snippet}

<aside>
  <p class="viewing">Viewing <strong>{total}</strong></p>
  <input class="filter" placeholder="Filter" bind:value={filter} />

  {#each above as section (section.key)}
    <section>
      <button class="head" onclick={() => onCollapse(section.key, !collapsed[section.key])}>
        <span class="caret">{collapsed[section.key] ? '›' : '⌄'}</span>
        <span class="icon" aria-hidden="true">{section.icon}</span>
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
                  <span class="tick" aria-hidden="true">✓</span>
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
  <section>
    <div class="row head-row" oncontextmenu={(e) => onRemoteMenu(e, null)} role="presentation">
      <button
        class="head"
        onclick={() => onCollapse('remote', !collapsed['remote'])}
        title="Remotes"
      >
        <span class="caret">{collapsed['remote'] ? '›' : '⌄'}</span>
        <span class="icon"><HostMark kind={sectionHost} /></span>
        Remote
        <span class="count">{groups.remote.length}</span>
      </button>
      <button
        class="dots"
        title="Add or manage remotes"
        onclick={(e) => onRemoteMenu(e, null)}
      >⋮</button>
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
            <span class="caret">{collapsed[key] ? '›' : '⌄'}</span>
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
          >⋮</button>
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
  <section>
    <button class="head" onclick={() => onCollapse('stashes', !collapsed['stashes'])}>
      <span class="caret">{collapsed['stashes'] ? '›' : '⌄'}</span>
      <span class="icon" aria-hidden="true">⤓</span>
      Stashes
      <span class="count">{stashes.length}</span>
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
                <span class="text">{elideRef(stash.name, 24)}</span>
              </button>
              <button
                class="dots"
                title="What can be done with {stash.name}"
                onclick={(e) => onStashMenu(e, stash)}
              >⋮</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#each below as section (section.key)}
    <section>
      <button class="head" onclick={() => onCollapse(section.key, !collapsed[section.key])}>
        <span class="caret">{collapsed[section.key] ? '›' : '⌄'}</span>
        <span class="icon" aria-hidden="true">{section.icon}</span>
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
    <section>
      <button class="head" onclick={() => onCollapse('prs', !collapsed['prs'])}>
        <span class="caret">{collapsed['prs'] ? '›' : '⌄'}</span>
        <span class="icon" aria-hidden="true">⇄</span>
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
    <section>
      <button class="head" onclick={() => onCollapse('submodules', !collapsed['submodules'])}>
        <span class="caret">{collapsed['submodules'] ? '›' : '⌄'}</span>
        <span class="icon" aria-hidden="true">◱</span>
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
                  <span class="tick" aria-hidden="true">{sub.initialised ? '✓' : '↓'}</span>
                  <span class="text">{sub.path}</span>
                </span>
                <button
                  class="dots"
                  title="What can be done with this submodule"
                  onclick={(e) => onSubmoduleMenu(e, sub)}
                >⋮</button>
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
  .viewing {
    margin: var(--space-1) var(--space-1) var(--space-2);
    font-size: 11px; text-transform: uppercase; letter-spacing: 0.05em; color: var(--fg-2);
  }
  .viewing strong { color: var(--fg-0); font-weight: 700; font-size: 12px; }
  .icon {
    width: 1.1em; color: var(--fg-2); flex: 0 0 auto;
    display: inline-flex; align-items: center; justify-content: center;
  }
  .filter {
    width: 100%; box-sizing: border-box; font: inherit; font-size: 12px;
    padding: 3px var(--space-2); margin-bottom: var(--space-2);
    border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  .filter::placeholder { color: var(--fg-2); }
  .filter:focus { border-color: var(--accent); }

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
    width: 100%; font: inherit; font-size: 10px; font-weight: 700; text-transform: uppercase;
    letter-spacing: 0.07em; color: var(--fg-2);
    background: var(--bg-1); border: 0; padding: var(--space-2) var(--space-1); cursor: pointer;
    border-radius: var(--radius-1);
  }
  .head:hover { color: var(--fg-1); }
  /* A remote is a row inside the section, so it is indented and set in the ordinary face: it
     names a thing, where the section above it names a kind. */
  .head.remote {
    padding-left: var(--space-3); text-transform: none; letter-spacing: 0;
    font-size: 12px; font-weight: 600; color: var(--fg-1);
  }
  .head.remote:hover { color: var(--fg-0); background: var(--bg-2); }
  /* A pill rather than a bare number: a count is a different kind of thing from the name
     beside it, and at this size only shape says so. */
  .count {
    margin-left: auto; color: var(--fg-2); font-size: 10px; font-weight: 600;
    background: var(--bg-2); border-radius: 999px; padding: 0 6px; min-width: 18px;
    text-align: center; flex: 0 0 auto;
  }
  .caret { width: 1em; color: var(--fg-2); flex: 0 0 auto; }
  ul { list-style: none; margin: 0 0 var(--space-2); padding: 0; }
  ul.nested { margin-left: var(--space-3); }
  .ref {
    display: flex; align-items: center; gap: var(--space-2);
    flex: 1; min-width: 0; text-align: left; font: inherit; font-size: 12px;
    padding: 3px var(--space-2) 3px var(--space-4);
    background: var(--bg-1); border: 0; border-radius: var(--radius-1); cursor: pointer;
    color: var(--fg-1); overflow: hidden;
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
    flex: 0 0 auto; font: inherit; font-size: 14px; line-height: 1; cursor: pointer;
    padding: 0 var(--space-2); background: var(--bg-1); border: 0; color: var(--fg-2);
    visibility: hidden;
  }
  .row:hover .dots, .row.current .dots { visibility: visible; }
  .dots:hover { color: var(--fg-0); }
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
  .num { flex: 0 0 auto; color: var(--fg-2); font-size: 11px; }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .state {
    flex: 0 0 auto; width: 13px; height: 13px; line-height: 13px; text-align: center;
    border-radius: 50%; font-size: 9px; font-weight: 700; color: #fff;
  }
  .state.open { background: var(--ok); }
  .state.draft { background: var(--fg-2); }
  .state.merged { background: var(--accent); }
  .state.closed { background: var(--danger); }
  /* The drop target, outlined rather than filled so the branch name stays readable under it. */
  .ref.over { outline: 2px solid var(--accent); outline-offset: -2px; border-radius: 3px; }
  .tick { color: var(--accent); flex: 0 0 auto; }
  .track { margin-left: auto; font-size: 10px; color: var(--fg-2); flex: 0 0 auto; }
  .none { margin: 0 0 var(--space-2) var(--space-4); font-size: 11px; color: var(--fg-2); }
  .more, .link {
    display: block; text-align: left; cursor: pointer;
    padding: 3px var(--space-4); font-size: 11px; color: var(--accent);
    background: var(--bg-1); border: 0; font-family: inherit;
  }
  .link { display: inline; padding: 0; }
  .more { width: 100%; }
  .more:hover, .link:hover { text-decoration: underline; }
</style>
