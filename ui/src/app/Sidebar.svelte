<script lang="ts">
  import type { PullRequest } from '../ipc/commands';
  import type { Remote, Submodule } from '../ipc/types';
  import type { PlacedRef, RefGroups } from '../state/refs.svelte';
  import { elideRef } from './path';

  const {
    groups,
    head,
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
    onInitAllSubmodules,
  }: {
    groups: RefGroups;
    head: string | null;
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
    /** Shows a submodule inside this tab. */
    onOpenSubmodule: (path: string) => void;
    /** `source` was dragged onto `target`; the shell decides what that means. */
    onDropRef: (source: string, target: string) => void;
    onOpenPullRequest: (pr: PullRequest) => void;
    /** Right-click on a remote, or on the section itself when `remote` is null. */
    onRemoteMenu: (event: MouseEvent, remote: string | null) => void;
    /** Fetches a working copy for every submodule that has none. */
    onInitAllSubmodules: () => void;
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
  // Remote and tag lists run to hundreds on a real repository, so they start closed as they do
  // in the reference; local branches are what people look at.
  let collapsed = $state<Record<string, boolean>>({ remote: true, tags: true });

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

  // Section order follows the reference's left panel: Local, Remote, Stashes, then Tags. The
  // remote section is rendered on its own because a remote is a thing with a menu, not a row.
  const above = $derived([{ key: 'local', title: 'Local', icon: '🖿', refs: shown(groups.local) }]);
  const below = $derived([
    { key: 'stashes', title: 'Stashes', icon: '⤓', refs: shown(groups.stashes) },
    { key: 'tags', title: 'Tags', icon: '🏷', refs: shown(groups.tags) },
  ]);

  const total = $derived(
    groups.local.length + groups.remote.length + groups.tags.length + groups.stashes.length,
  );
</script>

{#snippet refRow(r: PlacedRef, label: string, draggable: boolean)}
  <li>
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
  </li>
{/snippet}

<aside>
  <p class="viewing">Viewing <strong>{total}</strong></p>
  <input class="filter" placeholder="Filter" bind:value={filter} />

  {#each above as section (section.key)}
    <section>
      <button class="head" onclick={() => (collapsed[section.key] = !collapsed[section.key])}>
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

  <!--
    Remotes carry their own row, because a remote is a thing with a URL that can be edited,
    renamed and removed — not merely a prefix on a branch name.
  -->
  <section>
    <button
      class="head"
      onclick={() => (collapsed['remote'] = !collapsed['remote'])}
      oncontextmenu={(e) => onRemoteMenu(e, null)}
      title="Right-click to add or manage remotes"
    >
      <span class="caret">{collapsed['remote'] ? '›' : '⌄'}</span>
      <span class="icon" aria-hidden="true">☁</span>
      Remote
      <span class="count">{groups.remote.length}</span>
    </button>
    {#if !collapsed['remote']}
      {#if byRemote.size === 0}
        <p class="none">
          No remotes. <button class="link" onclick={(e) => onRemoteMenu(e, null)}>Add one</button>
        </p>
      {/if}
      {#each [...byRemote] as [name, refs] (name)}
        {@const key = `remote:${name}`}
        <button
          class="head remote"
          onclick={() => (collapsed[key] = !collapsed[key])}
          oncontextmenu={(e) => onRemoteMenu(e, name)}
          title={`${name}\n${urlOf(name)}\nRight-click for details`}
        >
          <span class="caret">{collapsed[key] ? '›' : '⌄'}</span>
          <span class="icon" aria-hidden="true">⌂</span>
          <span class="text">{name}</span>
          <span class="count">{refs.length}</span>
        </button>
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

  {#each below as section (section.key)}
    <section>
      <button class="head" onclick={() => (collapsed[section.key] = !collapsed[section.key])}>
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
      <button class="head" onclick={() => (collapsed['prs'] = !collapsed['prs'])}>
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
      <button class="head" onclick={() => (collapsed['submodules'] = !collapsed['submodules'])}>
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
            <li>
              <!--
                An uninitialised submodule is still worth clicking: it has no working copy yet,
                and offering to fetch one is more use than a row that does nothing.
              -->
              <button
                class="ref"
                class:current={sub.path === openSubmodule}
                class:absent={!sub.initialised}
                onclick={() => onOpenSubmodule(sub.path)}
                title={sub.initialised
                  ? `${sub.url || sub.name}\nOpen it here`
                  : `${sub.url || sub.name}\nNo working copy yet — click to fetch one`}
              >
                <span class="tick" aria-hidden="true">{sub.initialised ? '✓' : '↓'}</span>
                <span class="text">{sub.path}</span>
              </button>
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
  .icon { width: 1.1em; color: var(--fg-2); flex: 0 0 auto; }
  .filter {
    width: 100%; box-sizing: border-box; font: inherit; font-size: 12px;
    padding: 3px var(--space-2); margin-bottom: var(--space-2);
    border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  .filter::placeholder { color: var(--fg-2); }
  .filter:focus { border-color: var(--accent); }

  section + section { border-top: 1px solid var(--border); }
  .head {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; font: inherit; font-size: 10px; font-weight: 700; text-transform: uppercase;
    letter-spacing: 0.07em; color: var(--fg-2);
    background: none; border: 0; padding: var(--space-2) var(--space-1); cursor: pointer;
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
    width: 100%; text-align: left; font: inherit; font-size: 12px;
    padding: 3px var(--space-2) 3px var(--space-4);
    background: none; border: 0; border-radius: var(--radius-1); cursor: pointer;
    color: var(--fg-1); overflow: hidden;
  }
  .text { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis;
          white-space: nowrap; }
  .ref:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  .ref:disabled { color: var(--fg-2); cursor: default; }
  /* Declared but not fetched. Dimmed, but still a live target: clicking it fetches one. */
  .ref.absent { color: var(--fg-2); }
  .ref.absent .tick { color: var(--fg-2); }
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
    background: none; border: 0; font-family: inherit;
  }
  .link { display: inline; padding: 0; }
  .more { width: 100%; }
  .more:hover, .link:hover { text-decoration: underline; }
</style>
