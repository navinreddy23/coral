<script lang="ts">
  import { hasFlag, oidOf, RowFlag, type Frame } from '../graph/frame';
  import GraphCanvas from '../graph/GraphCanvas.svelte';
  import {
    DEFAULT_METRICS,
    firstRowFor,
    GRAPH_COLUMN_PX,
    REFS_COLUMN_PX,
    spacerHeight,
  } from '../graph/layout';
  import { initialRepo, open } from '../ipc/commands';
  import { GraphState } from '../state/graph.svelte';
  import { RefsState } from '../state/refs.svelte';
  import { ThemeState } from '../state/theme.svelte';
  import { SelectionState } from '../state/selection.svelte';
  import { WorktreeState } from '../state/worktree.svelte';
  import Staging from './Staging.svelte';
  import Details from './Details.svelte';
  import Sidebar from './Sidebar.svelte';
  import Toolbar from './Toolbar.svelte';
  import type { RepoInfo } from '../ipc/types';

  const graph = new GraphState();
  const theme = new ThemeState();
  const refs = new RefsState();
  const selection = new SelectionState();
  const worktree = new WorktreeState();
  let showWip = $state(false);
  let info = $state<RepoInfo | null>(null);
  let error = $state<string | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(600);
  let scroller = $state<HTMLDivElement | null>(null);

  const headName = $derived(
    info && info.head.kind !== 'detached' ? info.head.name : null,
  );

  function pick(row: number) {
    if (!graph.frame || !info) return;
    showWip = false;
    void selection.select(info.path, row, oidOf(graph.frame, row));
  }

  function pickWip() {
    showWip = true;
    selection.clear();
    if (info) void worktree.load(info.path);
  }

  /** What the WIP row summarises: how many files are waiting, staged or not. */
  const wipCount = $derived(worktree.status?.entries.length ?? 0);

  /** Scrolls a row into view, used when a ref is picked in the sidebar. */
  function reveal(row: number) {
    if (!graph.frame || !scroller) return;
    // Above the height cap a row is a fraction of a pixel, so the target is the fraction of
    // the scrollable range rather than the row's pixel offset.
    const total = graph.frame.rowCount;
    const height = spacerHeight(total, DEFAULT_METRICS);
    const lastTop = Math.max(1, total - Math.floor(viewport / DEFAULT_METRICS.rowHeight));
    const fraction = Math.max(0, row - 3) / lastTop;
    scroller.scrollTo({ top: Math.min(height - viewport, fraction * (height - viewport)) });
  }

  async function load(path: string) {
    error = null;
    try {
      info = await open(path);
      await graph.open(info.path);
      await refs.load(info.path);
      await worktree.load(info.path);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  void initialRepo().then(load);

  /** Rows currently worth putting in the DOM. Never the whole graph. */
  function windowRows(frame: Frame | null): number[] {
    if (!frame) return [];
    const perScreen = Math.ceil(viewport / DEFAULT_METRICS.rowHeight);
    const first = firstRowFor(scrollTop, viewport, frame.rowCount, DEFAULT_METRICS);
    const last = Math.min(frame.rowCount - 1, first + perScreen + 2);
    const out: number[] = [];
    for (let r = first; r <= last; r++) out.push(r);
    return out;
  }


  const rows = $derived(windowRows(graph.frame));
  /** Where the drawn rows sit, given the scroll position they were chosen for. */
  const windowTop = $derived(rows.length > 0 ? scrollTop : 0);

  // Only rows that are on screen are worth an object read.
  $effect(() => {
    if (rows.length > 0) void graph.loadMetadata(rows[0] ?? 0, rows.length);
  });

  function when(seconds: number): string {
    const delta = Date.now() / 1000 - seconds;
    const hours = delta / 3600;
    if (hours < 24) return `${Math.max(1, Math.round(hours))}h`;
    const days = hours / 24;
    if (days < 365) return `${Math.round(days)}d`;
    return `${(days / 365).toFixed(1)}y`;
  }
</script>

<main>
  <header>
    <h1>Coral</h1>
    {#if info}
      <span class="path mono">{info.path}</span>
      <span class="chip">{info.head.kind === 'detached' ? 'detached' : info.head.name}</span>
      <span class="chip">git {info.gitVersion}</span>
      {#if graph.frame}
        <span class="chip">{graph.frame.totalRows.toLocaleString()} commits</span>
      {/if}
      {#if graph.provisional}<span class="chip warn">provisional order</span>{/if}
    {/if}
    <button class="theme" onclick={() => theme.toggle()} title="Switch theme">
      {theme.current === 'light' ? 'Dark' : 'Light'}
    </button>
  </header>

  {#if info}
    <Toolbar
      repo={info.path.split('/').pop() ?? info.path}
      branch={headName ?? 'detached'}
      busy={worktree.busy}
      onAction={() => {}}
    />
  {/if}

  {#if graph.transportWarning}
    <p class="banner">{graph.transportWarning}</p>
  {/if}
  {#if error}
    <p class="banner error">{error}</p>
  {:else if graph.error}
    <p class="banner error">{graph.error}</p>
  {/if}

  {#if graph.loading && !graph.frame}
    <p class="muted">Walking the graph…</p>
  {:else if graph.frame}
    <div class="body">
    <Sidebar groups={refs.groups} head={headName} onSelect={reveal} />
    <div
      class="graph"
      bind:this={scroller}
      onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
      bind:clientHeight={viewport}
    >
      <div class="columns">
        <span class="col refs">Branch / Tag</span>
        <span class="col graph-col">Graph</span>
        <span class="col message">Commit message</span>
      </div>
      {#if worktree.dirty}
        <button class="row wip" class:selected={showWip} onclick={pickWip}>
          <span class="cell refs"></span>
          <span class="cell graph-col"><span class="wip-node"></span></span>
          <span class="cell message">
            <span class="summary">WIP on {headName ?? 'HEAD'}</span>
            <span class="detail">{wipCount} file{wipCount === 1 ? '' : 's'}</span>
          </span>
        </button>
      {/if}
      <div
        class="spacer"
        style:height="{spacerHeight(graph.frame.rowCount, DEFAULT_METRICS)}px"
      >
        <div class="lanes">
          <GraphCanvas frame={graph.frame} {scrollTop} height={viewport} />
        </div>
        <ul class="rows">
          {#each rows as row (row)}
            <li
              class="row"
              style:top="{windowTop + (row - (rows[0] ?? 0)) * DEFAULT_METRICS.rowHeight}px"
              class:merge={hasFlag(graph.frame.rowFlags[row] ?? 0, RowFlag.Merge)}
              class:selected={selection.row === row}
            >
              <button class="hit" onclick={() => pick(row)} aria-label="Select commit"></button>
              <span class="cell refs">
                {#each (refs.byRow.get(row) ?? []).slice(0, 2) as label (label.name)}
                  <span class="pill" class:head={label.short === headName}>{label.short}</span>
                {/each}
                {#if (refs.byRow.get(row) ?? []).length > 2}
                  <span class="pill more">+{(refs.byRow.get(row) ?? []).length - 2}</span>
                {/if}
              </span>
              <span class="cell graph-col"></span>
              <span class="cell message">
                <span class="summary">{graph.meta.get(row)?.summary ?? ''}</span>
                <span class="detail">{graph.meta.get(row)?.author ?? ''}</span>
                <span class="age">{when(graph.frame.times[row] ?? 0)}</span>
                <span class="sha mono">{oidOf(graph.frame, row).slice(0, 8)}</span>
              </span>
            </li>
          {/each}
        </ul>
      </div>
    </div>
    {#if showWip}
      <aside class="wip-panel"><Staging {worktree} /></aside>
    {:else}
      <Details detail={selection.detail} loading={selection.loading} error={selection.error} />
    {/if}
    </div>
  {/if}
</main>

<style>
  :root { --refs-col: 150px; --graph-col: 120px; }
  main { display: flex; flex-direction: column; height: 100%; }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  h1 { font-size: 15px; font-weight: 600; margin: 0; color: var(--accent); }
  .path { color: var(--fg-1); font-size: 12px; }
  .chip {
    font-size: 11px; padding: 2px var(--space-2); border-radius: 3px;
    background: var(--bg-2); color: var(--fg-1);
  }
  .chip.warn { background: var(--lane-3); color: var(--bg-0); }
  .theme {
    margin-left: auto; font: inherit; font-size: 11px; cursor: pointer;
    padding: 2px var(--space-2); border-radius: 3px;
    border: 1px solid var(--border); background: var(--bg-2); color: var(--fg-1);
  }
  .theme:hover { background: var(--bg-3); }
  .banner { margin: 0; padding: var(--space-2) var(--space-4); background: var(--bg-2); color: var(--fg-1); font-size: 12px; }
  .banner.error { color: var(--danger); }
  .muted { padding: var(--space-4); color: var(--fg-2); }

  .body { display: flex; flex: 1; min-height: 0; }
  .graph { flex: 1; overflow-y: auto; position: relative; }

  /* Column headers, matching the row grid below so the two cannot drift apart. */
  .columns, .row, .wip {
    display: grid;
    grid-template-columns: var(--refs-col) var(--graph-col) 1fr;
    align-items: center;
  }
  .columns {
    position: sticky; top: 0; z-index: 2;
    height: 22px; padding: 0 var(--space-3);
    background: var(--bg-1); border-bottom: 1px solid var(--border);
    font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--fg-2);
  }
  .col { overflow: hidden; }

  .spacer { position: relative; }
  /* The canvas tracks the scroll position rather than being as tall as the graph: a canvas
     millions of pixels high exhausts GPU texture memory. */
  .lanes {
    position: sticky; top: 22px; float: left; height: 0;
    margin-left: calc(var(--refs-col) + var(--space-3)); pointer-events: none;
  }
  .rows { list-style: none; margin: 0; padding: 0; }

  .row, .wip {
    position: absolute; left: 0; right: 0; height: var(--row-h);
    padding: 0 var(--space-3);
    font-size: 12px; color: var(--fg-1);
    border: 0; background: none; font-family: inherit; text-align: left;
  }
  .wip { position: sticky; top: 22px; z-index: 1; cursor: pointer; background: var(--bg-0); }
  .row:hover, .wip:hover { background: var(--bg-1); }
  .row.selected, .wip.selected { background: var(--accent-soft); }
  /* The whole row is the target; a button laid over it keeps that keyboard-reachable without
     nesting interactive elements inside one another. */
  .hit {
    position: absolute; inset: 0; width: 100%; height: 100%;
    background: none; border: 0; padding: 0; margin: 0; cursor: pointer;
  }
  .cell { min-width: 0; display: flex; align-items: center; gap: var(--space-2); }
  .cell.refs { justify-content: flex-end; padding-right: var(--space-2); }
  .cell.message { gap: var(--space-3); }

  .pill {
    flex: 0 0 auto; font-size: 11px; line-height: 1.5; padding: 0 var(--space-2);
    border-radius: 3px; border: 1px solid var(--border);
    background: var(--bg-2); color: var(--fg-1);
    max-width: 9em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pill.head { border-color: var(--accent); color: var(--accent); font-weight: 600; }
  .pill.more { color: var(--fg-2); }

  .summary {
    flex: 0 1 auto; min-width: 0; color: var(--fg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .row.merge .summary { color: var(--fg-0); }
  .detail {
    flex: 0 1 auto; min-width: 0; color: var(--fg-2);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .age { margin-left: auto; flex: 0 0 auto; color: var(--fg-2); }
  .sha { flex: 0 0 auto; color: var(--fg-2); }

  .wip-node {
    width: 10px; height: 10px; border-radius: 50%;
    border: 2px dashed var(--fg-2); margin-left: var(--space-1);
  }

  .wip-panel {
    width: 340px; flex: 0 0 auto; overflow-y: auto;
    border-left: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-3);
  }
</style>
