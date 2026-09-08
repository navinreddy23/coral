<script lang="ts">
  import Icon from './Icon.svelte';
  import Changes, { kindOf, markOf } from './Changes.svelte';
  import { buildTree } from '../diff/tree';
  import type { StatusEntry } from '../ipc/types';
  import type { CommitState } from '../state/commit.svelte';
  import type { WorktreeState } from '../state/worktree.svelte';
  import type { Grouping } from '../state/views.svelte';

  const {
    worktree,
    commit: draft,
    branch,
    openPath,
    grouping,
    onGrouping,
    onOpenFile,
    onDiscard,
    onFileMenu,
  }: {
    worktree: WorktreeState;
    /**
     * The message being written. Held by the window, because this panel is unmounted whenever
     * the selection moves off the working copy and a half-written message must survive that.
     */
    commit: CommitState;
    /** What the changes are on, which is what the header names. */
    branch: string | null;
    /** The file whose diff is on screen, so the list can mark it. */
    openPath: string | null;
    /** How both lists are arranged. Remembered by the window, not by this component. */
    grouping: Grouping;
    onGrouping: (grouping: Grouping) => void;
    /** Opens a working-tree file's diff; `staged` decides which half is shown. */
    onOpenFile: (path: string, staged: boolean) => void;
    /**
     * Asks to throw the given changes away.
     *
     * The panel never discards anything itself. This is the one action here that destroys
     * work, and the window owns the dialog that has to be answered first.
     */
    onDiscard: (entries: StatusEntry[]) => void;
    /**
     * What can be done with one file, asked for by right-clicking it.
     *
     * Raised rather than answered here: deleting a file is the one thing in this panel that
     * cannot be undone, and the window owns both the menu and the question that precedes it.
     */
    onFileMenu: (event: MouseEvent, entry: StatusEntry, staged: boolean) => void;
  } = $props();

  /**
   * git's own convention, and what every review tool wraps at.
   *
   * Shown as a countdown rather than enforced: a summary is not refused for being long, since
   * the commit is the user's to write.
   */
  const LIMIT = 72;

  const total = $derived(worktree.staged.length);
  const canCommit = $derived(draft.ready(worktree));

  /** The summary field, so the window can ask for the caret without reaching into the DOM. */
  let summaryField = $state<HTMLInputElement | null>(null);
  // Only when asked. The panel appears whenever the working copy is selected, and taking the
  // caret every time it did would put keystrokes meant for the list into the message.
  let focusedAt = 0;
  $effect(() => {
    if (draft.focusTick === focusedAt) return;
    focusedAt = draft.focusTick;
    summaryField?.focus();
  });

  async function record() {
    await worktree.commit(draft.message, draft.amend);
    if (!worktree.error) draft.clear();
  }

  /** Collapsed directories, per side, so expanding one list leaves the other alone. */
  let closedUnstaged = $state<Record<string, boolean>>({});
  let closedStaged = $state<Record<string, boolean>>({});

  const unstagedTree = $derived(buildTree(worktree.unstaged));
  const stagedTree = $derived(buildTree(worktree.staged));

  function expandAll(which: 'unstaged' | 'staged') {
    if (which === 'unstaged') closedUnstaged = {};
    else closedStaged = {};
  }

  function collapseAll(which: 'unstaged' | 'staged') {
    const paths: Record<string, boolean> = {};
    const walk = (nodes: ReturnType<typeof buildTree<StatusEntry>>) => {
      for (const node of nodes) {
        if (node.kind === 'dir') {
          paths[node.path] = true;
          walk(node.children);
        }
      }
    };
    walk(which === 'unstaged' ? unstagedTree : stagedTree);
    if (which === 'unstaged') closedUnstaged = paths;
    else closedStaged = paths;
  }

  const anyClosed = $derived(Object.values(closedUnstaged).some(Boolean));
</script>

<div class="panel">
  <header>
    <span class="count">
      {worktree.dirty ? worktree.status?.entries.length ?? 0 : 'No'} file
      change{(worktree.status?.entries.length ?? 0) === 1 ? '' : 's'}
    </span>
    {#if branch}
      <span class="on">on</span><span class="branch">{branch}</span>
    {/if}
    <div class="toggle">
      <button class:on={grouping === 'path'} onclick={() => onGrouping('path')}>Path</button>
      <button class:on={grouping === 'tree'} onclick={() => onGrouping('tree')}>Tree</button>
    </div>
    <!--
      Trailing, and last. It is the only control in this panel that takes something away, and
      it used to be the first thing the pointer reached on its way in from the graph.
    -->
    <button
      class="discard"
      disabled={!worktree.dirty || worktree.busy}
      title="Discard every change in the working copy"
      aria-label="Discard all changes"
      onclick={() => onDiscard(worktree.status?.entries ?? [])}
    ><Icon name="trash" size={14} /></button>
  </header>

  {#if worktree.error}
    <p class="error">{worktree.error}</p>
  {/if}

  {#if worktree.conflicted.length > 0}
    <!-- Above both lists: nothing else in the panel can be finished until these are settled. -->
    <h3 class="conflict">Conflicts ({worktree.conflicted.length})</h3>
    <ul class="flat">
      {#each worktree.conflicted as e (e.path)}
        <li>
          <button class="file" onclick={() => onOpenFile(e.path, false)} title={e.path}>
            <span class="mark conflict">!</span>
            <span class="name">{e.path}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <!--
    Unstaged first, then staged: work moves down the panel in the direction it moves through
    git, and the list being edited is the one at the top where the eye starts.
  -->
  <section>
    <h3>
      <span class="caret"><Icon name="chevronDown" size={13} /></span>
      Unstaged files ({worktree.unstaged.length})
      {#if grouping === 'tree' && worktree.unstaged.length > 0}
        <button
          class="expand"
          onclick={() => (anyClosed ? expandAll('unstaged') : collapseAll('unstaged'))}
        >{anyClosed ? 'Expand all' : 'Collapse all'}</button>
      {/if}
      {#if worktree.unstaged.length > 0}
        <button
          class="all"
          onclick={() => worktree.stage(worktree.unstaged.map((e) => e.path), true)}
        >Stage all changes</button>
      {/if}
    </h3>
    {#if worktree.unstaged.length === 0}
      <p class="empty">Nothing unstaged.</p>
    {:else if grouping === 'tree'}
      <Changes
        nodes={unstagedTree}
        staged={false}
        {openPath}
        closed={closedUnstaged}
        onToggleDir={(p) => (closedUnstaged[p] = !closedUnstaged[p])}
        onAct={(paths) => worktree.stage(paths, true)}
        onOpen={(p) => onOpenFile(p, false)}
        onMenu={(event, path) => {
          const entry = worktree.unstaged.find((e) => e.path === path);
          if (entry) onFileMenu(event, entry, false);
        }}
      />
    {:else}
      <ul class="flat">
        {#each worktree.unstaged as e (e.path)}
          <li oncontextmenu={(event) => onFileMenu(event, e, false)}>
            <button
              class="file"
              class:open={e.path === openPath}
              onclick={() => onOpenFile(e.path, false)}
              title={e.path}
            >
              <span class="mark {kindOf(e, false)}">{markOf(e, false)}</span>
              <span class="name">{e.path}</span>
            </button>
            <button class="act" title="Stage it" onclick={() => worktree.stage([e.path], true)}>+</button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <!-- The two halves are different places, not one list with a heading in the middle: a file
       moves between them, and the rule is what makes the move visible. -->
  <hr class="between" />

  <section>
    <h3>
      <span class="caret"><Icon name="chevronDown" size={13} /></span>
      Staged files ({worktree.staged.length})
      {#if worktree.staged.length > 0}
        <button
          class="all"
          onclick={() => worktree.stage(worktree.staged.map((e) => e.path), false)}
        >Unstage all changes</button>
      {/if}
    </h3>
    {#if worktree.staged.length === 0}
      <p class="empty">Nothing staged. A commit needs something in here.</p>
    {:else if grouping === 'tree'}
      <Changes
        nodes={stagedTree}
        staged={true}
        {openPath}
        closed={closedStaged}
        onToggleDir={(p) => (closedStaged[p] = !closedStaged[p])}
        onAct={(paths) => worktree.stage(paths, false)}
        onOpen={(p) => onOpenFile(p, true)}
        onMenu={(event, path) => {
          const entry = worktree.staged.find((e) => e.path === path);
          if (entry) onFileMenu(event, entry, true);
        }}
      />
    {:else}
      <ul class="flat">
        {#each worktree.staged as e (e.path)}
          <li oncontextmenu={(event) => onFileMenu(event, e, true)}>
            <button
              class="file"
              class:open={e.path === openPath}
              onclick={() => onOpenFile(e.path, true)}
              title={e.path}
            >
              <span class="mark {kindOf(e, true)}">{markOf(e, true)}</span>
              <span class="name">{e.path}</span>
            </button>
            <button class="act" title="Unstage it" onclick={() => worktree.stage([e.path], false)}>−</button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <div class="compose">
    <label class="amend">
      <input type="checkbox" bind:checked={draft.amend} />
      Amend the previous commit
    </label>
    <div class="field">
      <input
        class="summary"
        placeholder="Commit summary"
        bind:this={summaryField}
        bind:value={draft.summary}
      />
      <!-- Counts down rather than refusing: the commit is the user's to write. -->
      <span class="limit" class:over={draft.summary.length > LIMIT}>
        {LIMIT - draft.summary.length}
      </span>
    </div>
    <textarea class="description" rows="3" placeholder="Description" bind:value={draft.description}
    ></textarea>
    <button class="commit" disabled={!canCommit} onclick={record}>
      {#if draft.amend}
        Amend the previous commit
      {:else if total === 0}
        Stage something to commit
      {:else}
        Commit {total} file{total === 1 ? '' : 's'}
      {/if}
    </button>
  </div>
</div>

<style>
  /*
   * The whole height, and none of it wasted: the two lists take what is left after the header
   * and the commit box, half each, and scroll inside themselves. Before this the panel was a
   * column that stacked from the top, so a repository with two changed files left four fifths
   * of the panel empty and a repository with two hundred pushed the commit box off the end.
   */
  .panel {
    display: flex; flex-direction: column; gap: var(--space-2); font-size: 12px;
    flex: 1; min-height: 0;
  }
  .panel > section {
    flex: 1 1 0; min-height: 64px; display: flex; flex-direction: column;
  }
  /* The heading stays put; the list under it is what moves. */
  .panel > section > h3 { flex: 0 0 auto; }
  .panel > section > .flat,
  .panel > section > :global(.tree) { flex: 1 1 auto; min-height: 0; overflow-y: auto; }
  .compose { flex: 0 0 auto; }
  header {
    display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap;
    padding-bottom: var(--space-2); border-bottom: 1px solid var(--border);
  }
  .count { font-weight: 600; color: var(--fg-0); }
  .on { color: var(--fg-2); }
  .branch {
    font-size: var(--text-sm); padding: 1px 8px; border-radius: var(--radius-pill);
    background: var(--accent-soft); color: var(--accent);
    max-width: 12em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /*
   * Outlined in the danger colour rather than filled with it: filled, it would be the
   * brightest thing in the panel and the eye would land on it before the file list, which is
   * the wrong instinct to encourage above a list of unsaved work.
   */
  .discard {
    flex: 0 0 auto; display: flex; font: inherit; cursor: pointer;
    padding: 3px; border-radius: var(--radius-1);
    background: var(--bg-1); border: 1px solid var(--border); color: var(--fg-2);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .discard:hover:not(:disabled) {
    background: var(--danger-soft); border-color: var(--danger); color: var(--danger);
  }
  .discard:disabled { opacity: 0.4; cursor: default; }
  .toggle {
    display: flex; margin-left: auto;
    border: 1px solid var(--border); border-radius: var(--radius-1); overflow: hidden;
  }
  .toggle button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 2px var(--space-2);
    background: var(--bg-0); border: 0; color: var(--fg-2);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on { background: var(--accent); color: var(--accent-fg); font-weight: 600; }

  h3 {
    display: flex; align-items: center; gap: var(--space-1);
    font-size: var(--text-base); font-weight: 600; color: var(--fg-1);
    margin: var(--space-2) 0 var(--space-1);
  }
  h3.conflict { color: var(--danger); }
  .caret { color: var(--fg-2); }
  /* The whole-list action, set apart from the heading it belongs to rather than looking like
     part of the count. */
  .all {
    margin-left: auto; font: inherit; font-size: var(--text-xs); font-weight: 600;
    cursor: pointer;
    padding: 1px var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .all:hover { background: var(--bg-3); color: var(--fg-0); }
  /* Beside the whole-list action, and set as a link rather than a button: it changes how the
     list is drawn, where the button beside it changes what is in the index. */
  .expand {
    margin-left: auto; font: inherit; font-size: var(--text-xs); cursor: pointer;
    padding: 0; background: var(--bg-1); border: 0; color: var(--accent);
  }
  .expand:hover { text-decoration: underline; }
  /* With the link before it taking the leftover room, the button no longer needs to. */
  .expand + .all { margin-left: var(--space-2); }
  /* A real edge, not a hairline: the two halves are different places and a file moves between
     them, which the eye has to be able to see happen. */
  .between {
    border: 0; height: 1px; margin: var(--space-3) 0;
    background: var(--border-strong);
  }
  .empty { margin: 0 0 var(--space-2) 4px; color: var(--fg-2); font-size: var(--text-sm); }

  .flat { list-style: none; margin: 0; padding: 0; }
  .flat li { display: flex; align-items: center; }
  .flat li:hover { background: var(--bg-2); }
  .file {
    display: flex; align-items: center; gap: var(--space-2);
    flex: 1; min-width: 0; text-align: left; cursor: pointer;
    font: inherit; font-size: 12px; padding: 2px var(--space-2);
    background: var(--bg-1); border: 0; color: var(--fg-1);
  }
  .file.open { background: var(--accent-soft); color: var(--fg-0); }
  /* The end of a path identifies the file, so a long one is cut from the left. */
  .name {
    min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    direction: ltr;
  }
  .mark {
    flex: 0 0 auto; width: 12px; text-align: center;
    font-family: var(--font-mono); font-size: 11px; font-weight: 700;
  }
  .mark.added { color: var(--ok); }
  .mark.deleted { color: var(--danger); }
  .mark.renamed, .mark.copied, .mark.type_changed { color: var(--lane-5); }
  .mark.modified { color: var(--lane-1); }
  .mark.conflict { color: var(--danger); }
  .act {
    flex: 0 0 auto; font: inherit; font-size: 13px; line-height: 1; width: 1.6em;
    cursor: pointer; visibility: hidden;
    background: var(--bg-2); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1); margin-right: var(--space-1);
  }
  .flat li:hover .act { visibility: visible; }
  .act:hover { background: var(--bg-3); color: var(--fg-0); }

  /*
   * Pinned to the foot of the panel: the message is the last step, and it must not scroll away
   * under a long list of changes.
   *
   * Drawn as its own surface rather than as a rule with fields under it. Everything above is a
   * list of what has happened; this is the one place in the panel where something is written,
   * and giving it an edge and a fill of its own says which of the two it is.
   */
  .compose {
    position: sticky; bottom: 0; margin-top: var(--space-3);
    display: flex; flex-direction: column; gap: var(--space-2);
    padding: var(--space-3); border: 1px solid var(--border);
    border-radius: var(--radius-2);
    background: var(--bg-2);
  }
  .amend {
    display: flex; align-items: center; gap: var(--space-2);
    color: var(--fg-1); font-size: var(--text-base);
  }
  .field { position: relative; display: flex; }
  .summary, .description {
    flex: 1; min-width: 0; font: inherit; font-size: var(--text-base);
    padding: var(--space-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
    transition: border-color var(--fast) var(--ease);
  }
  .summary { padding-right: 3em; font-weight: 500; }
  .description { resize: vertical; line-height: var(--leading-body); }
  .summary:focus, .description:focus { border-color: var(--accent); }
  /*
   * Quiet until it matters. A number counting down beside every keystroke is a nag; what the
   * user needs to know is when the summary has grown past what git will show on one line.
   */
  .limit {
    position: absolute; right: var(--space-2); top: 50%; transform: translateY(-50%);
    font-size: var(--text-xs); color: var(--fg-2); font-variant-numeric: tabular-nums;
    opacity: 0; transition: opacity var(--fast) var(--ease); pointer-events: none;
  }
  .field:focus-within .limit, .limit.over { opacity: 1; }
  .limit.over { color: var(--warn); font-weight: 600; }
  .commit {
    font: inherit; font-size: var(--text-md); font-weight: 600;
    padding: var(--space-2); cursor: pointer;
    background: var(--accent); color: var(--accent-fg); border: 0;
    border-radius: var(--radius-1);
    transition: background var(--fast) var(--ease);
  }
  .commit:hover:not(:disabled) { background: var(--accent-hover); }
  .commit:disabled { background: var(--bg-3); color: var(--fg-2); cursor: default; }
  .error { color: var(--danger); margin: 0; }
</style>
