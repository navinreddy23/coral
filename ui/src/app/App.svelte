<script lang="ts">
  import { hasFlag, oidOf, RowFlag, type Frame } from '../graph/frame';
  import GraphCanvas from '../graph/GraphCanvas.svelte';
  import { DEFAULT_METRICS } from '../graph/layout';
  import { initialRepo, open } from '../ipc/commands';
  import { GraphState } from '../state/graph.svelte';
  import { RefsState } from '../state/refs.svelte';
  import { ThemeState } from '../state/theme.svelte';
  import Sidebar from './Sidebar.svelte';
  import type { RepoInfo } from '../ipc/types';

  const graph = new GraphState();
  const theme = new ThemeState();
  const refs = new RefsState();
  let info = $state<RepoInfo | null>(null);
  let error = $state<string | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(600);
  let scroller = $state<HTMLDivElement | null>(null);

  const headName = $derived(
    info && info.head.kind !== 'detached' ? info.head.name : null,
  );

  /** Scrolls a row into view, used when a ref is picked in the sidebar. */
  function reveal(row: number) {
    scroller?.scrollTo({ top: Math.max(0, (row - 3) * DEFAULT_METRICS.rowHeight) });
  }

  async function load(path: string) {
    error = null;
    try {
      info = await open(path);
      await graph.open(info.path);
      await refs.load(info.path);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  void initialRepo().then(load);

  /** Rows currently worth putting in the DOM. Never the whole graph. */
  function windowRows(frame: Frame | null): number[] {
    if (!frame) return [];
    const perScreen = Math.ceil(viewport / DEFAULT_METRICS.rowHeight);
    const first = Math.max(0, Math.floor(scrollTop / DEFAULT_METRICS.rowHeight));
    const last = Math.min(frame.rowCount - 1, first + perScreen + 2);
    const out: number[] = [];
    for (let r = first; r <= last; r++) out.push(r);
    return out;
  }

  const rows = $derived(windowRows(graph.frame));

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
      <div class="spacer" style:height="{graph.frame.rowCount * DEFAULT_METRICS.rowHeight}px">
        <div class="lanes" style:top="{0}px">
          <GraphCanvas frame={graph.frame} {scrollTop} height={viewport} />
        </div>
        <ul class="rows">
          {#each rows as row (row)}
            <li
              class="row"
              style:top="{row * DEFAULT_METRICS.rowHeight}px"
              class:merge={hasFlag(graph.frame.rowFlags[row] ?? 0, RowFlag.Merge)}
            >
              {#each refs.byRow.get(row) ?? [] as label (label.name)}
                <span class="pill" class:head={label.short === headName}>{label.short}</span>
              {/each}
              <span class="summary">{graph.meta.get(row)?.summary ?? ''}</span>
              <span class="author">{graph.meta.get(row)?.author ?? ''}</span>
              <span class="age">{when(graph.frame.times[row] ?? 0)}</span>
              <span class="sha mono">{oidOf(graph.frame, row).slice(0, 8)}</span>
            </li>
          {/each}
        </ul>
      </div>
    </div>
    </div>
  {/if}
</main>

<style>
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
  .spacer { position: relative; }
  /* The canvas tracks the scroll position rather than being as tall as the graph: a canvas
     millions of pixels high exhausts GPU texture memory. */
  .lanes { position: sticky; top: 0; float: left; height: 0; }
  .rows { list-style: none; margin: 0; padding: 0; }
  .row {
    position: absolute; left: 240px; right: 0; height: var(--row-h);
    display: flex; align-items: center; gap: var(--space-4);
    font-size: 12px; color: var(--fg-1);
  }
  .row.merge { color: var(--fg-0); }
  .pill {
    flex: 0 0 auto; font-size: 11px; padding: 1px var(--space-2);
    border-radius: 9px; border: 1px solid var(--border);
    background: var(--bg-2); color: var(--fg-1);
    max-width: 14em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pill.head { border-color: var(--accent); color: var(--accent); font-weight: 600; }
  .summary {
    flex: 1; min-width: 0; color: var(--fg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .author { width: 12em; flex: 0 0 auto; color: var(--fg-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .age { width: 3.5em; flex: 0 0 auto; color: var(--fg-2); text-align: right; }
  .sha { width: 6em; flex: 0 0 auto; color: var(--fg-2); text-align: right; padding-right: var(--space-4); }
</style>
