<script lang="ts">
  import type { PullRequest } from '../ipc/commands';
  import type { Submodule } from '../ipc/types';
  import type { PlacedRef, RefGroups } from '../state/refs.svelte';

  const {
    groups,
    head,
    submodules,
    pullRequests,
    pullRequestLabel,
    onSelect,
    onOpenSubmodule,
    onDropRef,
    onOpenPullRequest,
  }: {
    groups: RefGroups;
    head: string | null;
    submodules: Submodule[];
    /** Empty when the remote is not a recognised host, or no token is stored for it. */
    pullRequests: PullRequest[];
    /** What the host calls them: GitHub says pull, GitLab says merge. */
    pullRequestLabel: string;
    onSelect: (row: number) => void;
    /** Opens a submodule's working copy in its own tab. */
    onOpenSubmodule: (path: string) => void;
    /** `source` was dragged onto `target`; the shell decides what that means. */
    onDropRef: (source: string, target: string) => void;
    onOpenPullRequest: (pr: PullRequest) => void;
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

  /** Exact counts stop being useful past a point; the reference caps them at 99+. */
  function cap(n: number): string {
    return n > 99 ? '99+' : String(n);
  }

  function shown(refs: PlacedRef[]): PlacedRef[] {
    const q = filter.trim().toLowerCase();
    return q ? refs.filter((r) => r.short.toLowerCase().includes(q)) : refs;
  }

  const matchingSubmodules = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return q ? submodules.filter((s) => s.path.toLowerCase().includes(q)) : submodules;
  });

  // Section order follows the reference's left panel: Local, Remote, Stashes, then Tags.
  // Gitflow, pull requests, issues and actions come with their milestones.
  const sections = $derived([
    { key: 'local', title: 'Local', icon: '🖿', refs: shown(groups.local) },
    { key: 'remote', title: 'Remote', icon: '☁', refs: shown(groups.remote) },
    { key: 'stashes', title: 'Stashes', icon: '⤓', refs: shown(groups.stashes) },
    { key: 'tags', title: 'Tags', icon: '🏷', refs: shown(groups.tags) },
  ]);

  const total = $derived(
    groups.local.length + groups.remote.length + groups.tags.length + groups.stashes.length,
  );
</script>

<aside>
  <p class="viewing">Viewing <strong>{total}</strong></p>
  <input class="filter" placeholder="Filter" bind:value={filter} />
  {#each sections as section (section.key)}
    <section>
      <button class="head" onclick={() => (collapsed[section.key] = !collapsed[section.key])}>
        <span class="caret">{collapsed[section.key] ? '›' : '⌄'}</span>
        <span class="icon" aria-hidden="true">{section.icon}</span>
        {section.title}
        <span class="count">{section.refs.length}</span>
      </button>
      {#if !collapsed[section.key]}
        <ul>
          {#each (showingAll[section.key] ? section.refs : section.refs.slice(0, CAP)) as r (r.name)}
            <li>
              <button
                class="ref"
                class:current={r.short === head}
                class:dragging={dragging === r.short}
                class:over={over === r.short}
                draggable={section.key === 'local' || section.key === 'remote'}
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
                title={r.row === null ? 'not in the loaded graph' : r.name}
              >
                {#if r.short === head}<span class="tick" aria-hidden="true">✓</span>{/if}
                {r.short}
                {#if r.ahead > 0 || r.behind > 0}
                  <span class="track">{cap(r.ahead)}↑ {cap(r.behind)}↓</span>
                {/if}
              </button>
            </li>
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
      {#if !collapsed['submodules']}
        <ul>
          {#each matchingSubmodules as sub (sub.path)}
            <li>
              <button
                class="ref"
                disabled={!sub.initialised}
                onclick={() => onOpenSubmodule(sub.path)}
                title={sub.initialised
                  ? `${sub.url || sub.name}\nOpen in a new tab`
                  : `${sub.url || sub.name}\nNot initialised — run git submodule update --init`}
              >
                <span class="tick" aria-hidden="true">{sub.initialised ? '✓' : '·'}</span>
                {sub.path}
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
    font-size: 12px; color: var(--fg-2);
  }
  .viewing strong { color: var(--fg-0); font-weight: 600; }
  .icon { width: 1.1em; }
  .filter {
    width: 100%; box-sizing: border-box; font: inherit; font-size: 12px;
    padding: var(--space-1) var(--space-2); margin-bottom: var(--space-2);
    border: 1px solid var(--border); border-radius: 3px;
    background: var(--bg-0); color: var(--fg-0);
  }
  .head {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; font: inherit; font-size: 11px; text-transform: uppercase;
    letter-spacing: 0.04em; color: var(--fg-2);
    background: none; border: 0; padding: var(--space-2) var(--space-1); cursor: pointer;
  }
  .count { margin-left: auto; color: var(--fg-2); }
  .caret { width: 1em; }
  ul { list-style: none; margin: 0 0 var(--space-2); padding: 0; }
  .ref {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; font: inherit; font-size: 12px;
    padding: 3px var(--space-2) 3px var(--space-4);
    background: none; border: 0; border-radius: 3px; cursor: pointer;
    color: var(--fg-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .ref:hover:not(:disabled) { background: var(--bg-3); }
  .ref:disabled { color: var(--fg-2); cursor: default; }
  .ref.current { color: var(--fg-0); font-weight: 600; background: var(--accent-soft); }
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
  .track { margin-left: auto; font-size: 11px; color: var(--fg-2); }
  .more {
    display: block; width: 100%; text-align: left; cursor: pointer;
    padding: 3px var(--space-4); font-size: 11px; color: var(--accent);
    background: none; border: 0; font-family: inherit;
  }
  .more:hover { background: var(--bg-3); }
</style>
