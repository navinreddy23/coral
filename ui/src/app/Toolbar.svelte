<script lang="ts">
  const {
    repo,
    submodule,
    branch,
    busy,
    comparing,
    terminalOpen,
    onAction,
    onLeaveSubmodule,
    onPullMenu,
    onPushMenu,
  }: {
    repo: string;
    /** The submodule being looked at inside this tab, or null for the repository itself. */
    submodule: string | null;
    branch: string;
    busy: boolean;
    /** Whether two commits are picked, which is what decides what the patch button does. */
    comparing: boolean;
    terminalOpen: boolean;
    onAction: (name: string) => void;
    onLeaveSubmodule: () => void;
    /** Opens the choice of how a pull should integrate, at the caret. */
    onPullMenu: (event: MouseEvent) => void;
    /** Opens the choice of what a push should send, at the caret. */
    onPushMenu: (event: MouseEvent) => void;
  } = $props();

  /**
   * Grouped as the reference groups them: history, then the remote, then the working copy,
   * then the terminal. A rule is drawn between groups rather than extra space, because the
   * toolbar has to stay one row wide on a narrow window.
   *
   * Everything here acts on the repository. Preferences do not, which is why they sit in the
   * window's own chrome beside the theme switch instead.
   */
  const groups = $derived([
    [
      { name: 'undo', label: 'Undo', glyph: '↶', hint: 'Undo the last ref change' },
      { name: 'redo', label: 'Redo', glyph: '↷', hint: 'Redo what was undone' },
    ],
    [
      { name: 'fetch', label: 'Fetch', glyph: '⟳', hint: 'Fetch and prune' },
      { name: 'pull', label: 'Pull', glyph: '↓', hint: 'Pull, fast-forward only' },
      { name: 'push', label: 'Push', glyph: '↑', hint: 'Push, setting upstream if needed' },
    ],
    [
      { name: 'branch', label: 'Branch', glyph: '⑂', hint: 'Create a branch here' },
      { name: 'stash', label: 'Stash', glyph: '⤓', hint: 'Stash the working copy' },
      { name: 'pop', label: 'Pop', glyph: '⤒', hint: 'Apply the latest stash and drop it' },
      {
        name: 'patch',
        label: 'Patch',
        glyph: 'P',
        hint: comparing
          ? 'Write the commits between the two picked ones out as patch files'
          : 'Apply a patch file somebody sent',
      },
    ],
    [
      {
        name: 'terminal',
        label: 'Terminal',
        glyph: '>_',
        hint: terminalOpen ? 'Hide the terminal (Ctrl+`)' : 'Open a terminal here (Ctrl+`)',
      },
    ],
  ]);

  /** The last path segment, which is what a submodule is called. */
  const crumb = $derived(submodule?.split('/').filter(Boolean).at(-1) ?? null);
</script>

<div class="toolbar">
  <!--
    The breadcrumb, as the reference shows it: the repository, then the submodule being looked
    at inside it, then the branch. The submodule carries its own way out, since it is a step
    into the tab rather than a tab of its own.
  -->
  <div class="where">
    <span class="step">
      <span class="label">repository</span>
      <span class="value">{repo}</span>
    </span>
    {#if crumb}
      <span class="sep" aria-hidden="true">›</span>
      <span class="step sub">
        <button class="leave" onclick={onLeaveSubmodule} title="Back to {repo}">×</button>
        <span class="col">
          <span class="label">submodule</span>
          <span class="value" title={submodule}>{crumb}</span>
        </span>
      </span>
    {/if}
    <span class="sep" aria-hidden="true">›</span>
    <span class="step">
      <span class="label">branch</span>
      <span class="value">{branch}</span>
    </span>
  </div>

  <div class="actions">
    {#each groups as group, i (i)}
      {#if i > 0}<span class="rule" aria-hidden="true"></span>{/if}
      {#each group as action (action.name)}
        <!--
          The label sits above the glyph, as the reference sets them: a row of unlabelled
          symbols is unreadable, and reading the word first is what makes the symbol mean
          something the second time.
        -->
        <button
          class="action"
          class:on={action.name === 'terminal' && terminalOpen}
          disabled={busy && action.name !== 'terminal'}
          title={action.hint}
          onclick={() => onAction(action.name)}
        >
          <span class="name">{action.label}</span>
          <span class="glyph">{action.glyph}</span>
        </button>
        {#if action.name === 'pull'}
          <!-- How a pull integrates is a real choice, and the reference puts it here rather
               than only in a menu somewhere else. -->
          <button class="caret" disabled={busy} title="Choose how to pull" onclick={onPullMenu}>
            ▾
          </button>
        {/if}
        {#if action.name === 'push'}
          <!-- Tags do not travel with a push; git sends them only when they are asked for. -->
          <button class="caret" disabled={busy} title="Choose what to push" onclick={onPushMenu}>
            ▾
          </button>
        {/if}
      {/each}
    {/each}
  </div>

  <!-- Balances the breadcrumb, so the actions sit on the window's centre line. -->
  <div class="trailing"></div>
</div>

<style>
  /*
   * Three columns, the outer two of equal weight, so the actions land on the centre of the
   * window rather than on the centre of whatever the breadcrumb left over. With `margin: auto`
   * in a two-item flex row they drift right by half the breadcrumb's width, and by more as the
   * repository's name grows.
   */
  .toolbar {
    display: grid; grid-template-columns: 1fr auto 1fr; align-items: center;
    gap: var(--space-4);
    height: 46px; box-sizing: border-box; padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  .where { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .trailing { min-width: 0; }
  .step, .col { display: flex; flex-direction: column; line-height: 1.15; min-width: 0; }
  .step.sub { flex-direction: row; align-items: center; gap: var(--space-1); }
  /*
   * The two lines of the breadcrumb paint their own background, like every other text surface
   * in the window: on a composited layer WebKit antialiases with subpixel precision only where
   * it knows what is behind, and the branch name here is the boldest text in the toolbar, so
   * it is where the fallback to grayscale shows first.
   */
  .label, .value, .sep { background: var(--bg-1); }
  .label {
    font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--fg-2);
  }
  /* Wide enough for a real branch name. `gitlab-ci-local-support` is 23 characters and came
     out clipped at 14em; the column is a grid fraction, so a name longer than the window can
     hold still elides rather than pushing the actions off centre. */
  /* The line box has to hold the whole face, not just the x-height. Eliding needs
     `overflow: hidden`, and at the 1.15 the column sets, that clipped the tails off `g`, `p`
     and `y` — which most branch names have one of. */
  .value {
    font-size: 13px; font-weight: 600; line-height: 1.5; color: var(--fg-0);
    max-width: 24em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .sep { color: var(--fg-2); flex: 0 0 auto; }
  /* The way out of the submodule sits on the crumb itself, which is where the reference puts
     it and the only place it reads as belonging to that step rather than to the toolbar. */
  .leave {
    flex: 0 0 auto; font: inherit; font-size: 13px; line-height: 1; cursor: pointer;
    padding: 1px 4px; border-radius: var(--radius-1);
    background: var(--bg-1); border: 0; color: var(--fg-2);
  }
  .leave:hover { background: var(--bg-3); color: var(--danger); }

  /* Centred in the window, not pushed to one end: this is the row of things the user reaches
     for, and it belongs where the eye already is. */
  .actions { display: flex; align-items: center; gap: 2px; }
  .rule {
    width: 1px; height: 22px; margin: 0 var(--space-2);
    background: var(--border); flex: 0 0 auto;
  }
  .action {
    display: flex; flex-direction: column; align-items: center; gap: 2px;
    min-width: 54px; padding: 3px var(--space-2); line-height: 13px;
    font: inherit; cursor: pointer;
    background: var(--bg-1); border: 1px solid transparent; border-radius: var(--radius-1);
    color: var(--fg-1);
  }
  .action:hover:not(:disabled) {
    background: var(--bg-2); border-color: var(--border); color: var(--fg-0);
  }
  /* Pressed reads as pressed rather than as another hover: the surface goes under the page
     instead of above it. */
  .action:active:not(:disabled) { background: var(--bg-3); }
  /* A toggle that is on stays lit, so the terminal button says whether the pane is showing. */
  .action.on {
    background: var(--accent-soft); border-color: var(--accent); color: var(--fg-0);
  }
  .action:disabled { color: var(--fg-2); cursor: default; opacity: 0.5; }
  .name {
    font-size: 10px; text-transform: uppercase; letter-spacing: 0.04em;
  }
  /*
   * Stroked, because these are font glyphs and most arrows have no bold cut: asking for a
   * heavier weight changed nothing, and at 15px the hairlines all but disappeared against the
   * bar. The stroke is drawn in the glyph's own colour, so it thickens rather than outlines.
   */
  .glyph {
    font-size: 15px; line-height: 1; color: var(--accent);
    -webkit-text-stroke: 0.7px currentColor;
  }
  /* Sits against the button it belongs to rather than in the gap between two. */
  .caret {
    align-self: flex-end; margin: 0 var(--space-1) 5px -4px;
    font: inherit; font-size: 10px; line-height: 1; cursor: pointer;
    padding: 2px; background: var(--bg-1); border: 0; color: var(--fg-2);
  }
  .caret:hover:not(:disabled) { color: var(--fg-0); }
  .caret:disabled { opacity: 0.4; cursor: default; }
  .action:disabled .glyph { color: inherit; }
</style>
