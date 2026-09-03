<script lang="ts">
  import { covers, hasFlag, localRow, oidOf, RowFlag, type Frame } from '../graph/frame';
  import GraphCanvas from '../graph/GraphCanvas.svelte';
  import { initialsOf } from '../graph/initials';
  import Splitter from './Splitter.svelte';
  import { PANE_LIMITS, PanesState } from '../state/panes.svelte';
  import DiffView from './DiffView.svelte';
  import { DiffState } from '../state/diff.svelte';
  import { ActionsState } from '../state/actions.svelte';
  import MergeTool from './MergeTool.svelte';
  import { MergeState } from '../state/merge.svelte';
  import Palette, { type Command } from './Palette.svelte';
  import type { Action } from '../ipc/commands';
  import {
    DEFAULT_METRICS,
    firstRowFor,
    GRAPH_COLUMN_PX,
    listTop,
    REFS_COLUMN_PX,
    spacerHeight,
  } from '../graph/layout';
  import { initialRepo, open, pickRepository } from '../ipc/commands';
  import { GraphState } from '../state/graph.svelte';
  import { RefsState } from '../state/refs.svelte';
  import { ThemeState } from '../state/theme.svelte';
  import { SelectionState } from '../state/selection.svelte';
  import { WorktreeState } from '../state/worktree.svelte';
  import Staging from './Staging.svelte';
  import Details from './Details.svelte';
  import Sidebar from './Sidebar.svelte';
  import { TabsState } from '../state/tabs.svelte';
  import { isTextTarget, resolve, tabJump } from '../state/shortcuts';
  import Shortcuts from './Shortcuts.svelte';
  import TabBar from './TabBar.svelte';
  import Toolbar from './Toolbar.svelte';
  import type { RepoInfo } from '../ipc/types';

  const graph = new GraphState();
  const theme = new ThemeState();
  const refs = new RefsState();
  const selection = new SelectionState();
  const worktree = new WorktreeState();
  let showWip = $state(false);
  const tabs = new TabsState();
  let showHelp = $state(false);
  let showSidebar = $state(true);
  let showDetails = $state(true);

  /** Which shortcuts actually do something today; the help overlay dims the rest. */
  const LIVE = new Set([
    'select.next', 'select.previous', 'select.first', 'select.last',
    'stage.all', 'unstage.all', 'tab.new', 'tab.close', 'tab.next', 'tab.previous',
    'palette', 'repo.open',
    'panel.left', 'panel.detail', 'help',
  ]);

  function move(delta: number) {
    if (!graph.frame) return;
    const at = selection.row ?? -1;
    const next = Math.min(graph.totalRows - 1, Math.max(0, at + delta));
    pick(next);
    scrollToRow(next);
  }

  function onKey(event: KeyboardEvent) {
    const e = {
      key: event.key,
      ctrl: event.ctrlKey,
      shift: event.shiftKey,
      alt: event.altKey,
      meta: event.metaKey,
    };
    const jump = tabJump(e);
    if (jump !== null) {
      const target = tabs.session.tabs[jump - 1];
      if (target) void tabs.activate(target.id);
      event.preventDefault();
      return;
    }

    const binding = resolve(e, isTextTarget(event.target) ? 'message' : 'global');
    if (!binding || !LIVE.has(binding.id)) return;
    event.preventDefault();

    switch (binding.id) {
      case 'select.next': move(1); break;
      case 'select.previous': move(-1); break;
      case 'select.first': move(-Number.MAX_SAFE_INTEGER); break;
      case 'select.last': move(Number.MAX_SAFE_INTEGER); break;
      case 'stage.all': void worktree.stage(worktree.unstaged.map((f) => f.path), true); break;
      case 'unstage.all': void worktree.stage(worktree.staged.map((f) => f.path), false); break;
      case 'tab.new': void openAnother(); break;
      case 'tab.close': if (tabs.active) void tabs.close(tabs.active.id); break;
      case 'tab.next': cycleTab(1); break;
      case 'tab.previous': cycleTab(-1); break;
      case 'panel.left': showSidebar = !showSidebar; break;
      case 'panel.detail': showDetails = !showDetails; break;
      case 'help': showHelp = !showHelp; break;
      case 'palette': showPalette = !showPalette; break;
      case 'repo.open': void openAnother(); break;
      default: break;
    }
  }

  function cycleTab(delta: number) {
    const list = tabs.session.tabs;
    if (list.length === 0) return;
    const at = list.findIndex((t) => t.id === tabs.session.active);
    const next = list[(at + delta + list.length) % list.length];
    if (next) void tabs.activate(next.id);
  }
  let info = $state<RepoInfo | null>(null);
  let error = $state<string | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(600);
  const panes = new PanesState();
  const diff = new DiffState();
  const actions = new ActionsState();
  const merge = new MergeState();
  let showPalette = $state(false);
  let scroller = $state<HTMLDivElement | null>(null);

  const headName = $derived(
    info && info.head.kind !== 'detached' ? info.head.name : null,
  );

  function pick(row: number) {
    const local = localRow(graph.frame, row);
    if (local === null || !graph.frame || !info) return;
    showWip = false;
    void selection.select(info.path, row, oidOf(graph.frame, local));
  }

  /** Reloads everything after an operation finished, since it may have moved any of it. */
  async function reloadAll() {
    if (!info) return;
    const path = info.path;
    await Promise.all([refs.load(path), worktree.load(path), merge.load(path)]);
    await graph.open(path);
  }

  /** Every ref and where it points, as one string, to tell whether an action moved anything. */
  function refSignature(): string {
    return refs.all.map((r) => `${r.name}@${r.target}`).join('\u0000');
  }

  /**
   * Runs an action and reloads whatever it could have changed.
   *
   * Refs and status always, the graph only when a ref actually moved: rewalking 1.4M commits
   * after a stash that touched no ref would freeze the window for five seconds for nothing.
   */
  async function act(action: Action) {
    if (!info) return;
    const path = info.path;
    const before = refSignature();
    const outcome = await actions.run(path, action);
    if (!outcome) return;

    await Promise.all([refs.load(path), worktree.load(path), merge.load(path)]);
    const after = refSignature();
    if (before !== after) await graph.open(path);
  }

  /** Opens one of the selected commit's files in the diff viewer. */
  function openFile(file: string) {
    const rev = selection.detail?.commit.oid;
    if (!info || rev === undefined) return;
    void diff.open(info.path, rev, file);
  }

  /**
   * Everything the palette offers.
   *
   * Built from the repository rather than a fixed list, so a branch can be checked out, merged
   * or rebased onto by name without a submenu for each.
   */
  const commands = $derived.by<Command[]>(() => {
    const out: Command[] = [
      { id: 'fetch', label: 'Fetch', group: 'Remote', run: () => void act({ kind: 'fetch', remote: null }) },
      { id: 'pull', label: 'Pull (fast-forward only)', group: 'Remote', run: () => void act({ kind: 'pull', remote: null, mode: 'ffOnly' }) },
      { id: 'pull-rebase', label: 'Pull, rebasing', group: 'Remote', run: () => void act({ kind: 'pull', remote: null, mode: 'rebase' }) },
      { id: 'push', label: 'Push', group: 'Remote', run: () => void act({ kind: 'push', remote: null, setUpstream: true }) },
      { id: 'stash', label: 'Stash changes', group: 'Stash', run: () => void act({ kind: 'stashPush', message: null }) },
      { id: 'pop', label: 'Pop the latest stash', group: 'Stash', run: () => void act({ kind: 'stashApply', index: 0, pop: true }) },
      { id: 'undo', label: 'Undo', group: 'History', run: () => void act({ kind: 'undo' }) },
      { id: 'redo', label: 'Redo', group: 'History', run: () => void act({ kind: 'redo' }) },
      { id: 'theme', label: 'Toggle dark mode', group: 'View', run: () => theme.toggle() },
    ];

    for (const r of refs.groups.local) {
      if (r.short === headName) continue;
      out.push({ id: `co:${r.name}`, label: `Checkout ${r.short}`, group: 'Branch', run: () => void act({ kind: 'checkout', rev: r.short }) });
      out.push({ id: `merge:${r.name}`, label: `Merge ${r.short} into ${headName ?? 'HEAD'}`, group: 'Branch', run: () => void act({ kind: 'merge', rev: r.short }) });
      out.push({ id: `rebase:${r.name}`, label: `Rebase onto ${r.short}`, group: 'Branch', run: () => void act({ kind: 'rebase', onto: r.short }) });
    }
    for (const r of refs.groups.tags.slice(0, 200)) {
      out.push({ id: `co:${r.name}`, label: `Checkout tag ${r.short}`, group: 'Tag', run: () => void act({ kind: 'checkout', rev: r.short }) });
    }
    return out;
  });

  /**
   * A branch dropped onto another.
   *
   * The reference reads the gesture as "bring `source` into `target`", which needs `target`
   * checked out first — dropping onto a branch you are not on otherwise merges into the wrong
   * one silently. Dropping a local branch onto its remote counterpart pushes instead, which is
   * the one case where the gesture means something else entirely.
   */
  async function dropRef(source: string, target: string) {
    const pushing = target === `origin/${source}` || target.endsWith(`/${source}`);
    if (pushing) {
      await act({ kind: 'push', remote: null, setUpstream: true });
      return;
    }
    const how = window.prompt(`Bring ${source} into ${target}? Type "merge" or "rebase".`, 'merge');
    if (how === null) return;
    if (target !== headName) await act({ kind: 'checkout', rev: target });
    if (how.trim().toLowerCase() === 'rebase') await act({ kind: 'rebase', onto: source });
    else await act({ kind: 'merge', rev: source });
  }

  /** The toolbar's seven buttons, each the commonest form of its action. */
  function toolbarAction(name: string) {
    const branch = headName;
    switch (name) {
      case 'undo': return void act({ kind: 'undo' });
      case 'redo': return void act({ kind: 'redo' });
      case 'fetch': return void act({ kind: 'fetch', remote: null });
      case 'pull': return void act({ kind: 'pull', remote: null, mode: 'ffOnly' });
      // set-upstream on every push: it is a no-op once one is configured, and without it the
      // first push of a new branch fails with advice instead of pushing.
      case 'push': return void act({ kind: 'push', remote: null, setUpstream: true });
      case 'stash': return void act({ kind: 'stashPush', message: null });
      case 'pop': return void act({ kind: 'stashApply', index: 0, pop: true });
      case 'branch': {
        const name = window.prompt(`New branch from ${branch ?? 'HEAD'}`)?.trim();
        if (name) void act({ kind: 'branchCreate', name, at: null, checkout: true });
        return;
      }
      default:
        return;
    }
  }

  function pickWip() {
    showWip = true;
    selection.clear();
    if (info) void worktree.load(info.path);
  }

  /** What the WIP row summarises: how many files are waiting, staged or not. */
  const wipCount = $derived(worktree.status?.entries.length ?? 0);

  /** Scrolls a row into view, used when a ref is picked in the sidebar. */
  /**
   * Scrolls a row into view and selects it.
   *
   * Following a branch in the sidebar should land on that commit, not merely somewhere near
   * it: without the selection the detail panel still describes whatever was picked last, and
   * nothing on the row that was scrolled to says it is the one that was asked for.
   */
  async function reveal(row: number) {
    scrollToRow(row);
    // A ref can point anywhere in the history, which is very unlikely to be inside whatever
    // frame is loaded, so the rows have to arrive before there is anything to select.
    await graph.ensureRows(row, row);
    pick(row);
  }

  function scrollToRow(row: number) {
    if (!graph.frame || !scroller) return;
    // Above the height cap a row is a fraction of a pixel, so the target is the fraction of
    // the scrollable range rather than the row's pixel offset.
    const total = graph.totalRows;
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
      // A repository can be opened mid-merge, so the tool has to be there on arrival rather
      // than only after an action of ours stopped.
      await merge.load(info.path);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * Restores the session, then opens whatever it was left on.
   *
   * CORAL_REPO still wins when set, so the app can be pointed at a repository for
   * benchmarking without disturbing the saved tabs.
   */
  async function start() {
    await tabs.refresh();
    const requested = await initialRepo();
    if (requested !== '.') await tabs.open(requested);
    else if (!tabs.active && tabs.session.tabs.length === 0) await tabs.open(requested);
    // The effect below loads whatever ends up active; loading here as well would walk the
    // graph twice on launch, which on a large repository is two five-second walks.
  }
  void start();

  // Switching tabs loads that repository; nothing else in the shell needs to know.
  let loadedPath = $state('');
  $effect(() => {
    const path = tabs.active?.path;
    if (path && path !== loadedPath) {
      loadedPath = path;
      void load(path);
    }
  });

  async function openAnother() {
    const path = await pickRepository();
    if (path) await tabs.open(path);
  }

  /**
   * Opens a submodule in its own tab.
   *
   * A submodule is a repository in its own right, so it gets a tab rather than a mode of this
   * one; joining with the parent's path keeps it working when the parent was opened relatively.
   */
  async function openSubmodule(relative: string) {
    const parent = tabs.active?.path;
    if (!parent) return;
    await tabs.open(`${parent.replace(/\/+$/u, '')}/${relative}`);
  }

  /** Rows currently worth putting in the DOM. Never the whole graph. */
  function windowRows(frame: Frame | null): number[] {
    if (!frame) return [];
    const perScreen = Math.ceil(viewport / DEFAULT_METRICS.rowHeight);
    const first = firstRowFor(scrollTop, viewport, frame.totalRows, DEFAULT_METRICS);
    const last = Math.min(frame.totalRows - 1, first + perScreen + 2);
    const out: number[] = [];
    for (let r = first; r <= last; r++) out.push(r);
    return out;
  }


  const rows = $derived(windowRows(graph.frame));

  /**
   * Reading `graph.meta` here rather than inside the canvas keeps the redraw reactive: the
   * identity of this function changes whenever a metadata block lands, which is the signal the
   * canvas repaints on.
   */
  const nodeInitials = $derived.by(() => {
    const meta = graph.meta;
    return (row: number) => {
      const author = meta.get(row)?.author;
      return author === undefined ? null : initialsOf(author);
    };
  });


  // Only rows that are on screen are worth an object read, or a frame.
  $effect(() => {
    if (rows.length === 0) return;
    const first = rows[0] ?? 0;
    const last = rows[rows.length - 1] ?? first;
    void graph.ensureRows(first, last);
    void graph.loadMetadata(first, rows.length);
  });

  /**
   * The body as one dimmed line after the summary, as the reference shows it. Newlines become
   * a separator rather than being dropped, so a bullet list still reads as several points.
   */
  function flatten(body: string): string {
    return body
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.length > 0)
      .join(' | ');
  }

  function when(seconds: number): string {
    const delta = Date.now() / 1000 - seconds;
    const hours = delta / 3600;
    if (hours < 24) return `${Math.max(1, Math.round(hours))}h`;
    const days = hours / 24;
    if (days < 365) return `${Math.round(days)}d`;
    return `${(days / 365).toFixed(1)}y`;
  }
</script>

<svelte:window onkeydown={onKey} />

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

  <TabBar {tabs} onOpen={openAnother} />

  {#if info}
    <Toolbar
      repo={info.path.split('/').pop() ?? info.path}
      branch={headName ?? 'detached'}
      busy={worktree.busy || actions.busy}
      onAction={toolbarAction}
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
    <div
      class="body"
      style:--refs-col="{panes.widths.refs}px"
      style:--graph-col="{panes.widths.graph}px"
      style:--sidebar-w="{panes.widths.sidebar}px"
      style:--details-w="{panes.widths.details}px"
    >
    {#if showSidebar}
      <Sidebar
        groups={refs.groups}
        head={headName}
        submodules={refs.submodules}
        onSelect={reveal}
        onOpenSubmodule={openSubmodule}
        onDropRef={dropRef}
      />
      <Splitter
        label="Resize the sidebar"
        value={panes.widths.sidebar}
        min={PANE_LIMITS.sidebar.min}
        max={PANE_LIMITS.sidebar.max}
        onresize={(px) => panes.resize('sidebar', px)}
        onreset={() => panes.reset()}
      />
    {/if}
    {#if merge.inProgress}
      <!-- A stopped merge or rebase is the only thing that matters until it is settled, so it
           takes the main pane outright rather than sitting behind the graph. -->
      <MergeTool {merge} onDone={reloadAll} />
    {:else if diff.path !== null}
      <DiffView {diff} onClose={() => diff.close()} />
    {/if}
    <div
      class="graph"
      class:hidden={diff.path !== null || merge.inProgress}
      bind:this={scroller}
      onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
      bind:clientHeight={viewport}
    >
      <div class="columns">
        <span class="col refs">
          Branch / Tag
          <Splitter
            label="Resize the branch and tag column"
            value={panes.widths.refs}
            min={PANE_LIMITS.refs.min}
            max={PANE_LIMITS.refs.max}
            onresize={(px) => panes.resize('refs', px)}
            onreset={() => panes.reset()}
          />
        </span>
        <span class="col graph-col">
          Graph
          <Splitter
            label="Resize the graph column"
            value={panes.widths.graph}
            min={PANE_LIMITS.graph.min}
            max={PANE_LIMITS.graph.max}
            onresize={(px) => panes.resize('graph', px)}
            onreset={() => panes.reset()}
          />
        </span>
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
        style:height="{spacerHeight(graph.totalRows, DEFAULT_METRICS)}px"
      >
        <div class="lanes" style:top="{listTop(scrollTop)}px">
          <GraphCanvas
            frame={graph.frame}
            firstRow={rows[0] ?? 0}
            height={viewport}
            width={panes.widths.graph}
            initials={nodeInitials}
          />
        </div>
        <ul class="rows" style:top="{listTop(scrollTop)}px">
          {#each rows as row (row)}
            <!--
              A row the loaded frame does not reach is left blank rather than read out of the
              wrong end of a typed array, which would show another commit's date and hash.
            -->
            {@const local = localRow(graph.frame, row)}
            <li
              class="row"
              class:merge={hasFlag(graph.frame.rowFlags[local ?? -1] ?? 0, RowFlag.Merge)}
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
                <span class="detail">{flatten(graph.meta.get(row)?.body ?? '')}</span>
                {#if local !== null}
                  <span class="age">{when(graph.frame.times[local] ?? 0)}</span>
                  <span class="sha mono">{oidOf(graph.frame, local).slice(0, 8)}</span>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    </div>
    {#if showDetails}
      <Splitter
        label="Resize the detail panel"
        value={panes.widths.details}
        min={PANE_LIMITS.details.min}
        max={PANE_LIMITS.details.max}
        grows="left"
        onresize={(px) => panes.resize('details', px)}
        onreset={() => panes.reset()}
      />
      {#if showWip}
        <aside class="wip-panel"><Staging {worktree} /></aside>
      {:else}
        <Details
          detail={selection.detail}
          loading={selection.loading}
          error={selection.error}
          openPath={diff.path}
          onOpenFile={openFile}
        />
      {/if}
    {/if}
    </div>
  {/if}
</main>

{#if actions.report}
  <!-- The one place an action says what happened; it clears on the next one. -->
  <p class="status {actions.report.tone}">
    {actions.report.text}
    <button class="dismiss" onclick={() => actions.clear()} aria-label="Dismiss">✕</button>
  </p>
{/if}

{#if showPalette}
  <Palette {commands} onClose={() => (showPalette = false)} />
{/if}

{#if showHelp}
  <Shortcuts live={LIVE} onClose={() => (showHelp = false)} />
{/if}

<style>
  :root { --refs-col: 190px; --graph-col: 170px; }
  main { display: flex; flex-direction: column; height: 100%; }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    height: 44px; box-sizing: border-box; padding: 0 var(--space-4);
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
  .status {
    position: fixed; left: 0; right: 0; bottom: 0; z-index: 15; margin: 0;
    display: flex; align-items: center; gap: var(--space-2);
    padding: 4px var(--space-3); font-size: 12px;
    border-top: 1px solid var(--border); background: var(--bg-1); color: var(--fg-1);
  }
  .status.warn { color: var(--fg-0); background: var(--add-bg); }
  .status.error { color: var(--danger); background: var(--remove-bg); }
  .dismiss {
    margin-left: auto; font: inherit; cursor: pointer;
    background: none; border: 0; color: inherit;
  }
  .graph { flex: 1; overflow-y: auto; position: relative; background: var(--bg-0); }
  /* Hidden rather than unmounted: remounting would refetch the frame and lose the scroll
     position every time a file is opened and closed. */
  .graph.hidden { display: none; }

  /* Column headers, matching the row grid below so the two cannot drift apart. */
  .columns, .row, .wip {
    display: grid;
    grid-template-columns: var(--refs-col) var(--graph-col) 1fr;
    align-items: center;
  }
  .columns {
    position: sticky; top: 0; z-index: 2;
    height: 24px; padding: 0 var(--space-3);
    background: var(--bg-1); border-bottom: 1px solid var(--border);
    font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--fg-2);
  }
  .col { overflow: hidden; position: relative; display: flex; align-items: center; }
  /* The handle sits on the column's right edge and spans the header's full height. */
  .col :global(.splitter) {
    position: absolute; right: 0; top: 0; bottom: 0; margin: 0 -4px 0 0;
  }

  .spacer { position: relative; }
  /* The canvas tracks the scroll position rather than being as tall as the graph: a canvas
     millions of pixels high exhausts GPU texture memory. */
  /* Absolutely positioned at the same rounded offset the rows use, rather than sticky: one
     fewer composited layer in the scroller, and the lanes cannot drift half a pixel from the
     text they belong to. */
  .lanes {
    position: absolute; left: calc(var(--refs-col) + var(--space-3));
    height: 0; pointer-events: none;
  }
  /* The list is positioned once and the rows stack inside it in normal flow. Positioning each
     row individually put every one of them at its own computed offset; laying them out
     normally means only one element can be off, and it is snapped. */
  .rows { list-style: none; margin: 0; padding: 0; position: absolute; left: 0; right: 0; }

  .row, .wip {
    position: relative; height: var(--row-h);
    padding: 0 var(--space-3);
    font-size: 12px; color: var(--fg-1);
    border: 0; background: none; font-family: inherit; text-align: left;
  }
  .wip { position: sticky; top: 22px; z-index: 1; cursor: pointer; background: var(--bg-0); }
  .row:hover .cell.message, .wip:hover { background: var(--bg-1); }
  .row.selected .cell.message, .wip.selected { background: var(--accent-soft); }
  /*
   * The text columns paint an opaque background of their own. Over a transparent composited
   * layer WebKit drops from subpixel to grayscale antialiasing, which reads as soft — and
   * these rows sit above a canvas, which is what promotes the layer. The lane column stays
   * transparent so the canvas shows through it.
   */
  .cell.message, .cell.refs { background: var(--bg-0); }
  .cell.message {
    border-radius: 3px; padding: 0 var(--space-2);
    /* The row's own height, so the highlight is a band rather than a floating pill. */
    height: 100%;
  }
  /* The whole row is the target; a button laid over it keeps that keyboard-reachable without
     nesting interactive elements inside one another. */
  .hit {
    position: absolute; inset: 0; width: 100%; height: 100%;
    background: none; border: 0; padding: 0; margin: 0; cursor: pointer;
  }
  .cell { min-width: 0; display: flex; align-items: center; gap: var(--space-2); }
  /* Pills are clipped to their own column rather than spilling over the lanes. */
  .cell.refs { justify-content: flex-end; padding-right: var(--space-2); overflow: hidden; }
  .cell.message { gap: var(--space-3); }

  .pill {
    flex: 0 1 auto; min-width: 0; font-size: 11px; line-height: 1.5; padding: 0 var(--space-2);
    border-radius: 3px; border: 1px solid var(--border);
    background: var(--bg-2); color: var(--fg-1);
    max-width: 11em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pill.head { border-color: var(--accent); color: var(--accent); font-weight: 600; }
  .pill.more { color: var(--fg-2); }

  /* The summary takes its natural width and the dimmed body absorbs what is left. Letting
     both shrink equally gave the body most of the row, so summaries were cut to a few
     characters while their continuation ran on — the wrong half was being kept. */
  .summary {
    flex: 0 1 auto; min-width: 4em; max-width: 62%; color: var(--fg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .detail {
    flex: 1 1 0; min-width: 0; color: var(--fg-2);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .age { flex: 0 0 3.5em; color: var(--fg-2); text-align: right; }
  .sha { flex: 0 0 6em; color: var(--fg-2); text-align: right; }

  .wip-node {
    width: 10px; height: 10px; border-radius: 50%;
    border: 2px dashed var(--fg-2); margin-left: var(--space-1);
  }

  .wip-panel {
    width: var(--details-w, 340px); flex: 0 0 auto; overflow-y: auto;
    border-left: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-3);
  }
</style>
