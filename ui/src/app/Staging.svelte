<script lang="ts">
  import { t } from '../state/locale.svelte';
  import { tick, untrack } from 'svelte';
  import Icon from './Icon.svelte';
  import Splitter from './Splitter.svelte';
  import Changes, { kindOf, markOf } from './Changes.svelte';
  import LockMark from './LockMark.svelte';
  import { buildTree } from '../diff/tree';
  import type { LfsLock, StatusEntry } from '../ipc/types';
  import type { CommitState } from '../state/commit.svelte';
  import type { WorktreeState } from '../state/worktree.svelte';
  import type { Grouping } from '../state/views.svelte';
  import {
    fitHeight,
    LIST_FLOOR_PX,
    SECTION_LIMITS,
    type SectionsState,
  } from '../state/sections.svelte';

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
    onCommit,
    sections,
    locks = new Map(),
    onDraft = null,
    drafting = false,
  }: {
    worktree: WorktreeState;
    /**
     * The message being written. Held by the window, because this panel is unmounted whenever
     * the selection moves off the working copy and a half-written message must survive that.
     */
    commit: CommitState;
    /** Drafts a message with Claude, when that is turned on; null hides the button. */
    onDraft?: (() => void) | null;
    drafting?: boolean;
    /** What the changes are on, which is what the header names. */
    branch: string | null;
    /** Files locked on the LFS server, by path. */
    locks?: Map<string, LfsLock>;
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
    /**
     * Makes the commit.
     *
     * Raised rather than done here. The panel used to call the engine itself, so the window's
     * own commit — the one `Ctrl Enter` runs — was a second path through the same act, and
     * anything the window wanted to do about a refusal only happened down one of them. git
     * refusing for want of a name is exactly such a thing, and it is the first commit anybody
     * makes.
     */
    onCommit: () => void;
    /** How the panel's height is shared between its sections, and which are folded away. */
    sections: SectionsState;
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
  // A folded message box is opened first: the caret was asked for, and it cannot go into a
  // field that is not there.
  $effect(() => {
    if (draft.focusTick === focusedAt) return;
    focusedAt = draft.focusTick;
    untrack(() => sections.open('compose'));
    void tick().then(() => summaryField?.focus());
  });

  // Ticking amend from the graph's menu fills the message in; it is no use doing that out of
  // sight.
  $effect(() => {
    if (draft.amend) untrack(() => sections.open('compose'));
  });

  /*
   * A file whose diff is put on screen from elsewhere — the diff's own next and previous — is
   * shown in its list, so the list never hides what is open. Only on a change of file: showing
   * the panel again must not undo a fold the user made while that file was already open.
   */
  let seenPath = untrack(() => openPath);
  $effect(() => {
    const path = openPath;
    if (path === seenPath) return;
    seenPath = path;
    if (path === null) return;
    untrack(() => {
      const unstaged = worktree.unstaged.some((e) => e.path === path);
      const staged = worktree.staged.some((e) => e.path === path);
      if ((unstaged && !sections.collapsed.unstaged) || (staged && !sections.collapsed.staged)) {
        return;
      }
      if (unstaged) sections.open('unstaged');
      else if (staged) sections.open('staged');
    });
  });

  const closed = $derived(sections.collapsed);
  const bothLists = $derived(!closed.unstaged && !closed.staged);
  const anyList = $derived(!closed.unstaged || !closed.staged);

  let panelPx = $state(0);
  let unstagedPx = $state(0);
  let stagedPx = $state(0);
  let composePx = $state(0);

  /**
   * The two lists as flex weights. A fraction rather than a height, so a taller window gives
   * both lists more and the split stays where it was put.
   */
  const unstagedGrow = $derived(bothLists ? sections.sizes.unstagedShare : 1);
  const stagedGrow = $derived(bothLists ? 1 - sections.sizes.unstagedShare : 1);

  /** The height the lists must keep under the message box: a floor for each one open. */
  const listsFloor = $derived((bothLists ? 2 : 1) * LIST_FLOOR_PX + (bothLists ? 0 : 32));

  /** The message box's height as drawn: as dragged, unless the window cannot spare it. */
  const composeDrawn = $derived.by(() => {
    const want = sections.sizes.composeHeight;
    if (want === null || !anyList) return null;
    return fitHeight(want, panelPx - 48, listsFloor, SECTION_LIMITS.composeHeight.min);
  });

  /**
   * The tallest the box can be dragged here and now.
   *
   * A drag is stored no taller than this. The pointer can go on past the point the box stops
   * at, and storing where it went rather than where the box went meant that folding a list
   * later gave the room it freed to the message box rather than to the list left open.
   */
  const composeMax = $derived(
    panelPx > 0
      ? Math.max(SECTION_LIMITS.composeHeight.min, panelPx - 48 - listsFloor)
      : SECTION_LIMITS.composeHeight.max,
  );

  function resizeLists(px: number): void {
    const both = unstagedPx + stagedPx;
    if (both > 0) sections.resize('unstagedShare', px / both);
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

<div class="panel" bind:clientHeight={panelPx}>
  <header>
    <span class="count">
      {worktree.dirty
        ? t('staging.fileChanges', { n: worktree.status?.entries.length ?? 0 })
        : t('staging.noFileChanges')}
    </span>
    {#if branch}
      <span class="on">{t('staging.on')}</span><span class="branch" title={branch}>{branch}</span>
    {/if}
    <div class="toggle">
      <button class:on={grouping === 'path'} onclick={() => onGrouping('path')}>{t('staging.path')}</button>
      <button class:on={grouping === 'tree'} onclick={() => onGrouping('tree')}>{t('staging.tree')}</button>
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
  <section
    class:closed={closed.unstaged}
    style:flex-grow={closed.unstaged ? null : unstagedGrow}
    bind:clientHeight={unstagedPx}
  >
    <h3>
      <button
        class="fold"
        aria-expanded={!closed.unstaged}
        aria-controls="staging-unstaged"
        onclick={() => sections.toggle('unstaged')}
      >
        <span class="caret">
          <Icon name={closed.unstaged ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        <span class="title">{t('staging.unstaged')}</span>
        <span class="tally">({worktree.unstaged.length})</span>
      </button>
      {#if !closed.unstaged && grouping === 'tree' && worktree.unstaged.length > 0}
        <button
          class="expand"
          onclick={() => (anyClosed ? expandAll('unstaged') : collapseAll('unstaged'))}
        >{anyClosed ? t('staging.expandAll') : t('staging.collapseAll')}</button>
      {/if}
      {#if worktree.unstaged.length > 0}
        <button
          class="all"
          onclick={() => worktree.stage(worktree.unstaged.map((e) => e.path), true)}
        >{t('staging.stageAll')}</button>
      {/if}
    </h3>
    {#if !closed.unstaged}
    <div class="list" id="staging-unstaged">
    {#if worktree.unstaged.length === 0}
      <p class="empty">{t('staging.nothingUnstaged')}</p>
    {:else if grouping === 'tree'}
      <Changes
        nodes={unstagedTree}
        staged={false}
        {openPath}
        closed={closedUnstaged}
        onToggleDir={(p) => (closedUnstaged[p] = !closedUnstaged[p])}
        onAct={(paths) => worktree.stage(paths, true)}
        onOpen={(p) => onOpenFile(p, false)}
        {locks}
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
              <LockMark lock={locks.get(e.path)} />
            </button>
            <button class="act" title={t('staging.stageIt')} onclick={() => worktree.stage([e.path], true)}>+</button>
          </li>
        {/each}
      </ul>
    {/if}
    </div>
    {/if}
  </section>

  <!-- The two halves are different places, not one list with a heading in the middle: a file
       moves between them, and the edge is what makes the move visible. It is also the handle
       that shares the height out between them, while both are open. -->
  {#if bothLists}
    <div class="between">
      <Splitter
        vertical
        grip
      label="Resize the unstaged and staged lists"
        value={unstagedPx}
        min={0}
        max={unstagedPx + stagedPx}
        onresize={resizeLists}
        onreset={() => sections.reset('unstagedShare')}
      />
    </div>
  {:else}
    <hr class="rule" />
  {/if}

  <section
    class:closed={closed.staged}
    style:flex-grow={closed.staged ? null : stagedGrow}
    bind:clientHeight={stagedPx}
  >
    <h3>
      <button
        class="fold"
        aria-expanded={!closed.staged}
        aria-controls="staging-staged"
        onclick={() => sections.toggle('staged')}
      >
        <span class="caret">
          <Icon name={closed.staged ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        <span class="title">{t('staging.staged')}</span>
        <span class="tally">({worktree.staged.length})</span>
      </button>
      {#if worktree.staged.length > 0}
        <button
          class="all"
          onclick={() => worktree.stage(worktree.staged.map((e) => e.path), false)}
        >{t('staging.unstageAll')}</button>
      {/if}
    </h3>
    {#if !closed.staged}
    <div class="list" id="staging-staged">
    {#if worktree.staged.length === 0}
      <p class="empty">{t('staging.nothingStaged')}</p>
    {:else if grouping === 'tree'}
      <Changes
        nodes={stagedTree}
        staged={true}
        {openPath}
        closed={closedStaged}
        onToggleDir={(p) => (closedStaged[p] = !closedStaged[p])}
        onAct={(paths) => worktree.stage(paths, false)}
        onOpen={(p) => onOpenFile(p, true)}
        {locks}
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
              <LockMark lock={locks.get(e.path)} />
            </button>
            <button class="act" title={t('staging.unstageIt')} onclick={() => worktree.stage([e.path], false)}>−</button>
          </li>
        {/each}
      </ul>
    {/if}
    </div>
    {/if}
  </section>

  {#if anyList && !closed.compose}
    <Splitter
      vertical
      grows="left"
      grip
      label="Resize the commit message"
      value={sections.sizes.composeHeight ?? composePx}
      min={SECTION_LIMITS.composeHeight.min}
      max={composeMax}
      onresize={(px) => sections.resize('composeHeight', Math.min(px, composeMax))}
      onreset={() => sections.reset('composeHeight')}
    />
  {/if}

  <div
    class="compose"
    class:closed={closed.compose}
    class:fill={!anyList && !closed.compose}
    class:sized={composeDrawn !== null && !closed.compose}
    style:height={composeDrawn !== null && !closed.compose ? `${composeDrawn}px` : null}
    bind:clientHeight={composePx}
  >
    <h3>
      <button
        class="fold"
        aria-expanded={!closed.compose}
        aria-controls="staging-compose"
        onclick={() => sections.toggle('compose')}
      >
        <span class="caret">
          <Icon name={closed.compose ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        {t('staging.compose')}
      </button>
    </h3>
    {#if !closed.compose}
    <div class="fields" id="staging-compose">
    <div class="above">
      <label class="amend">
        <input type="checkbox" bind:checked={draft.amend} />
        {t('staging.amend')}
      </label>
      {#if onDraft !== null}
        <button
          class="draft"
          disabled={drafting || total === 0}
          onclick={onDraft}
          title="Write a message for what is staged with Claude, for you to read and change"
        >{drafting ? t('staging.drafting') : t('staging.draft')}</button>
      {/if}
    </div>
    <div class="field">
      <input
        class="summary"
        placeholder={t('staging.summary')}
        bind:this={summaryField}
        bind:value={draft.summary}
      />
      <!-- Counts down rather than refusing: the commit is the user's to write. -->
      <span class="limit" class:over={draft.summary.length > LIMIT}>
        {LIMIT - draft.summary.length}
      </span>
    </div>
    <textarea class="description" rows="3" placeholder={t('staging.description')} bind:value={draft.description}
    ></textarea>
    </div>
    {/if}
    <button class="commit" disabled={!canCommit} onclick={onCommit}>
      <!-- A disabled button says what is missing rather than what it would have done. -->
      {#if total === 0 && !draft.amend}
        {t('staging.needsStaged')}
      {:else if draft.summary.trim().length === 0}
        {t('staging.needsSummary')}
      {:else if draft.templated}
        {t('staging.needsTemplate')}
      {:else if draft.amend}
        {t('staging.amend')}
      {:else}
        {t('staging.commitFiles', { n: total })}
      {/if}
    </button>
  </div>
</div>

<style>
  .above { display: flex; align-items: center; gap: var(--space-2); }
  .above .amend { flex: 1; }
  .draft {
    flex: 0 0 auto; font: inherit; font-size: var(--text-sm); cursor: pointer; white-space: nowrap;
    padding: 1px var(--space-2); background: var(--bg-1); color: var(--fg-1);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .draft:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  .draft:disabled { opacity: 0.6; cursor: default; }
  /*
   * The whole height, and none of it wasted: the two lists take what is left after the header
   * and the commit box, half each, and scroll inside themselves. Before this the panel was a
   * column that stacked from the top, so a repository with two changed files left four fifths
   * of the panel empty and a repository with two hundred pushed the commit box off the end.
   */
  .panel {
    display: flex; flex-direction: column; gap: var(--space-2); font-size: var(--text-base);
    flex: 1; min-height: 0;
  }
  .panel > section {
    flex: 1 1 0; min-height: 64px; display: flex; flex-direction: column;
  }
  /* Folded to its heading, which gives its share of the height to whatever is still open. */
  .panel > section.closed { flex: 0 0 auto; min-height: 0; }
  /* The heading stays put; the list under it is what moves. */
  .panel > section > h3 { flex: 0 0 auto; }
  .panel > section > .list { flex: 1 1 auto; min-height: 0; overflow-y: auto; }
  .compose { flex: 0 0 auto; }
  /* Both lists folded: the message box is all there is, so it takes the panel. */
  .compose.fill { flex: 1 1 auto; min-height: 0; }
  .compose.sized { min-height: 0; }
  /*
   * One line, whatever the branch is called. It used to wrap, and a branch name like
   * `feature/shout` was enough to put the discard button on a line of its own under the count,
   * away from everything else in the header. The branch name gives way instead.
   */
  header {
    display: flex; align-items: center; gap: var(--space-2);
    padding-bottom: var(--space-2); border-bottom: 1px solid var(--border);
  }
  .count { font-weight: var(--weight-semibold); color: var(--fg-0); white-space: nowrap; }
  .on { color: var(--fg-2); }
  .branch {
    flex: 0 1 auto; min-width: 0;
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
    background: var(--bg-0); border: var(--hairline) solid var(--border); color: var(--fg-2);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .discard:hover:not(:disabled) {
    background: var(--danger-soft); border-color: var(--danger); color: var(--danger);
  }
  .discard:disabled { opacity: 0.4; cursor: default; }
  /* A segmented control: the choice that is on is a raised face in a sunken track. */
  .toggle {
    display: flex; margin-left: auto; gap: var(--space-0); padding: var(--space-0);
    background: var(--bg-1); border-radius: var(--radius-1);
  }
  .toggle button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 1px var(--space-2);
    background: var(--bg-1); border: 0; border-radius: var(--radius-0); color: var(--fg-2);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on {
    background: var(--bg-0); color: var(--fg-0); font-weight: var(--weight-semibold);
    box-shadow: 0 0 0 var(--hairline) var(--border);
  }

  h3 {
    display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-1);
    font-size: var(--text-xs); font-weight: var(--weight-semibold); color: var(--fg-2);
    text-transform: uppercase; letter-spacing: var(--tracking-label);
    margin: var(--space-2) 0 var(--space-1);
  }
  h3.conflict { color: var(--danger); }
  /* The heading is a label; the buttons on it are still sentences. */
  h3 button { text-transform: none; letter-spacing: normal; }
  .caret { color: var(--fg-2); display: flex; }
  /* The heading's own words are what fold the section, so it reads as a heading, not as a
     button sitting on one. */
  h3 .fold {
    display: flex; align-items: center; cursor: pointer;
    flex: 1 1 7em; min-width: 0; overflow: hidden;
    text-align: left; white-space: nowrap;
    font: inherit; color: inherit; text-transform: inherit; letter-spacing: inherit;
    padding: 0; margin: 0; background: none; border: 0; border-radius: var(--radius-0);
  }
  /* A narrow panel shortens the heading's words, keeping its count; only a panel too narrow
     for even that puts the buttons beside it on a line of their own. */
  h3 .fold .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  h3 .fold .tally { flex: 0 0 auto; margin-left: 0.3em; }
  h3 .fold .caret { margin-right: var(--space-1); }
  h3 .fold:hover { color: var(--fg-0); }
  h3 .fold:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  /* The whole-list action, set apart from the heading it belongs to rather than looking like
     part of the count. */
  .all {
    flex: 0 0 auto; white-space: nowrap;
    margin-left: auto; font: inherit; font-size: var(--text-xs); font-weight: var(--weight-semibold);
    cursor: pointer;
    padding: 1px var(--space-1); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .all:hover { background: var(--bg-3); color: var(--fg-0); }
  /* Beside the whole-list action, and set as a link rather than a button: it changes how the
     list is drawn, where the button beside it changes what is in the index. */
  .expand {
    flex: 0 0 auto; white-space: nowrap;
    margin-left: auto; font: inherit; font-size: var(--text-xs); cursor: pointer;
    padding: 0; background: var(--bg-0); border: 0; color: var(--accent);
  }
  .expand:hover { text-decoration: underline; }
  /* With the link before it taking the leftover room, the button no longer needs to. */
  .expand + .all { margin-left: var(--space-1); }
  /* A real edge, not a hairline: the two halves are different places and a file moves between
     them, which the eye has to be able to see happen. */
  .between, .rule {
    flex: 0 0 auto; border: 0; height: 1px; margin: var(--space-3) 0;
    background: var(--border-strong);
  }
  .empty { margin: 0 0 var(--space-2) 4px; color: var(--fg-2); font-size: var(--text-sm); }

  .flat { list-style: none; margin: 0; padding: 0; }
  .flat li { display: flex; align-items: center; }
  .flat li:hover, .flat li:hover .file:not(.open) { background: var(--bg-1); }
  .file {
    display: flex; align-items: center; gap: var(--space-2);
    flex: 1; min-width: 0; text-align: left; cursor: pointer;
    font: inherit; font-size: var(--text-base); padding: 2px var(--space-2);
    background: var(--bg-0); border: 0; border-radius: var(--radius-0); color: var(--fg-1);
    min-height: calc(var(--row-h) - 4px);
  }
  .file.open { background: var(--accent-soft); color: var(--fg-0); }
  /* The end of a path identifies the file, so a long one is cut from the left. */
  .name {
    min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    direction: ltr;
  }
  .mark {
    flex: 0 0 auto; width: 12px; text-align: center;
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); font-weight: var(--weight-bold);
  }
  .mark.added { color: var(--ok); }
  .mark.deleted { color: var(--danger); }
  .mark.renamed, .mark.copied, .mark.type_changed { color: var(--lane-5); }
  .mark.modified { color: var(--lane-1); }
  .mark.conflict { color: var(--danger); }
  .act {
    flex: 0 0 auto; font: inherit; font-size: var(--text-md); line-height: 1; width: 1.6em;
    cursor: pointer; visibility: hidden;
    background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius-0);
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
    position: sticky; bottom: 0;
    display: flex; flex-direction: column; gap: var(--space-2);
    padding: var(--space-3); border: var(--hairline) solid var(--border);
    border-radius: var(--radius-2);
    background: var(--bg-1);
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
  .summary { padding-right: 3em; font-weight: var(--weight-medium); }
  /* The divider above the box is how it is made taller; a second grip on the field would be
     two ways to do one thing, and only one of them would be remembered. */
  .description { resize: none; line-height: var(--leading-body); }
  .fields { display: flex; flex-direction: column; gap: var(--space-2); }
  /* Given a height, the description is what takes it: that is the field a longer message
     needs room in. */
  .compose.sized .fields, .compose.fill .fields { flex: 1 1 auto; min-height: 0; }
  .compose.sized .description, .compose.fill .description { flex: 1 1 auto; min-height: 2.6em; }
  .compose h3 { margin: 0; }
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
  .limit.over { color: var(--warn); font-weight: var(--weight-semibold); }
  .commit {
    font: inherit; font-size: var(--text-md); font-weight: var(--weight-semibold);
    padding: var(--space-2); cursor: pointer;
    background: var(--accent); color: var(--accent-fg); border: 0;
    border-radius: var(--radius-1);
    transition: background var(--fast) var(--ease);
  }
  .commit:hover:not(:disabled) { background: var(--accent-hover); }
  .commit:disabled { background: var(--bg-3); color: var(--fg-2); cursor: default; }
  .error { color: var(--danger); margin: 0; }
</style>
