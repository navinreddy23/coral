<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icon';
  import { keysOf } from '../state/shortcuts';

  const {
    repo,
    path,
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
    /** Where the repository is on disk. Shown on the crumb rather than in the title bar. */
    path: string;
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

  interface Action {
    name: string;
    label: string;
    icon: IconName;
    hint: string;
    /** The binding in `shortcuts.ts` whose keys belong in the tooltip, where there is one. */
    binding?: string;
  }

  /**
   * Grouped as git groups them: history, then the remote, then the working copy. A rule is
   * drawn between groups rather than extra space, so the row stays one wide on a narrow
   * window.
   *
   * Everything here acts on the repository. Preferences do not, which is why they sit in the
   * window's own chrome beside the theme switch instead. The terminal is the one thing here
   * that acts on neither, and it sits alone at the far end for that reason.
   */
  const groups = $derived<Action[][]>([
    [
      { name: 'undo', label: 'Undo', icon: 'undo', hint: 'the last ref change', binding: 'undo' },
      { name: 'redo', label: 'Redo', icon: 'redo', hint: 'what was undone', binding: 'redo' },
    ],
    [
      { name: 'fetch', label: 'Fetch', icon: 'fetch', hint: 'and prune', binding: 'fetch.all' },
      { name: 'pull', label: 'Pull', icon: 'pull', hint: 'fast-forward only' },
      { name: 'push', label: 'Push', icon: 'push', hint: 'setting the upstream if needed' },
    ],
    [
      {
        name: 'branch',
        label: 'Branch',
        icon: 'branch',
        hint: 'create one here',
        binding: 'branch.create',
      },
      { name: 'stash', label: 'Stash', icon: 'stashPush', hint: 'put the working copy aside' },
      { name: 'pop', label: 'Pop', icon: 'stashPop', hint: 'apply the latest stash and drop it' },
      {
        name: 'patch',
        label: 'Patch',
        icon: 'patch',
        hint: comparing
          ? 'write the commits between the two picked ones out as files'
          : 'apply a patch file somebody sent',
      },
    ],
  ]);

  const terminal = $derived<Action>({
    name: 'terminal',
    label: 'Terminal',
    icon: 'terminal',
    hint: terminalOpen ? 'hide the pane' : 'open one in this repository',
    binding: 'terminal',
  });

  /**
   * What a button says when the pointer rests on it.
   *
   * The name comes first because the buttons carry no label of their own now: this is the
   * only place the word appears, and burying it after an explanation would make the tooltip
   * something to read rather than something to glance at.
   */
  function tip(action: Action): string {
    const keys = action.binding === undefined ? null : keysOf(action.binding);
    return `${action.label} — ${action.hint}${keys === null ? '' : ` (${keys})`}`;
  }

  /** The last path segment, which is what a submodule is called. */
  const crumb = $derived(submodule?.split('/').filter(Boolean).at(-1) ?? null);
</script>

<div class="toolbar">
  <!--
    The repository, the submodule being looked at inside it, then the branch. The submodule
    carries its own way out, since it is a step into the tab rather than a tab of its own.
  -->
  <div class="where">
    <span class="step" title={path}>
      <span class="label">repository</span>
      <span class="value">{repo}</span>
    </span>
    {#if crumb}
      <span class="sep"><Icon name="chevronRight" size={12} /></span>
      <span class="step sub">
        <button class="leave" onclick={onLeaveSubmodule} title="Back to {repo}">
          <Icon name="close" size={12} />
        </button>
        <span class="col">
          <span class="label">submodule</span>
          <span class="value" title={submodule}>{crumb}</span>
        </span>
      </span>
    {/if}
    <span class="sep"><Icon name="chevronRight" size={12} /></span>
    <span class="step">
      <span class="label">branch</span>
      <span class="value">{branch}</span>
    </span>
  </div>

  <div class="actions">
    {#each groups as group, i (i)}
      {#if i > 0}<span class="rule" aria-hidden="true"></span>{/if}
      {#each group as action (action.name)}
        <button
          class="action"
          disabled={busy}
          title={tip(action)}
          aria-label={action.label}
          onclick={() => onAction(action.name)}
        >
          <Icon name={action.icon} size={17} />
        </button>
        {#if action.name === 'pull'}
          <!-- How a pull integrates is a real choice, and it belongs on the button that
               would otherwise make it silently. -->
          <button class="caret" disabled={busy} title="Choose how to pull" onclick={onPullMenu}>
            <Icon name="chevronDown" size={11} />
          </button>
        {/if}
        {#if action.name === 'push'}
          <!-- Tags do not travel with a push; git sends them only when they are asked for. -->
          <button class="caret" disabled={busy} title="Choose what to push" onclick={onPushMenu}>
            <Icon name="chevronDown" size={11} />
          </button>
        {/if}
      {/each}
    {/each}
  </div>

  <div class="trailing">
    <button
      class="action"
      class:on={terminalOpen}
      title={tip(terminal)}
      aria-label={terminal.label}
      aria-pressed={terminalOpen}
      onclick={() => onAction('terminal')}
    >
      <Icon name="terminal" size={17} />
    </button>
  </div>
</div>

<style>
  /*
   * Left to right rather than centred. The buttons carry no words now, so the row is half
   * what it was; a cluster of icons floated in the middle of a wide window reads as debris,
   * where a run of them against the breadcrumb reads as a toolbar.
   */
  .toolbar {
    display: flex; align-items: center; gap: var(--space-4);
    height: 40px; box-sizing: border-box; padding: 0 var(--space-3) 0 var(--space-4);
    border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  .where { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .step, .col { display: flex; flex-direction: column; line-height: 1.15; min-width: 0; }
  .step.sub { flex-direction: row; align-items: center; gap: var(--space-1); }
  /*
   * The two lines of the breadcrumb paint their own background, like every other text surface
   * in the window: on a composited layer WebKit antialiases with subpixel precision only where
   * it knows what is behind, and the branch name here is the boldest text in the toolbar, so
   * it is where the fallback to grayscale shows first.
   */
  .label, .value { background: var(--bg-1); }
  /* Clipped like the value under it. Only the value was, so in a narrow window the crumb's
     label overflowed its own box and painted over the next one: "REPOSITORY" and "BRANCH"
     came out as "REPOSITOR'BRANCH". */
  .label {
    font-size: var(--text-xs); text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--fg-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /* Wide enough for a real branch name. `gitlab-ci-local-support` is 23 characters and came
     out clipped at 14em; the column shrinks, so a name longer than the window can hold still
     elides rather than pushing the actions off the row.
     The line box has to hold the whole face, not just the x-height: eliding needs
     `overflow: hidden`, and at 1.15 that clipped the tails off `g`, `p` and `y`. */
  .value {
    font-size: var(--text-md); font-weight: 600; line-height: 1.45; color: var(--fg-0);
    max-width: 22em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .sep { color: var(--border-strong); flex: 0 0 auto; display: flex; align-items: center; }
  /* The way out of the submodule sits on the crumb itself, which is the only place it reads
     as belonging to that step rather than to the toolbar. */
  .leave {
    flex: 0 0 auto; display: flex; font: inherit; cursor: pointer;
    padding: 2px; border-radius: var(--radius-1);
    background: var(--bg-1); border: 0; color: var(--fg-2);
    transition: color var(--fast) var(--ease), background var(--fast) var(--ease);
  }
  .leave:hover { background: var(--danger-soft); color: var(--danger); }

  .actions { display: flex; align-items: center; gap: 1px; min-width: 0; }
  /* A rule rather than a gap: the groups have to stay told apart on a window narrow enough
     that spacing would be the first thing squeezed. */
  .rule {
    width: 1px; height: 18px; margin: 0 var(--space-2);
    background: var(--border); flex: 0 0 auto;
  }
  /* The terminal is the one control here that acts on the window rather than the repository,
     so it sits at the far end with the row's whole width between them. */
  .trailing { margin-left: auto; display: flex; align-items: center; }

  .action {
    display: flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; padding: 0; cursor: pointer;
    font: inherit;
    background: var(--bg-1); border: 1px solid transparent; border-radius: var(--radius-1);
    color: var(--fg-1);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .action:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  /* Pressed reads as pressed rather than as another hover: the surface goes under the page
     instead of above it. */
  .action:active:not(:disabled) { background: var(--bg-3); transition: none; }
  /* A toggle that is on stays lit, so the terminal button says whether the pane is showing. */
  .action.on { background: var(--accent-soft); color: var(--accent); }
  .action:disabled { color: var(--fg-2); cursor: default; opacity: 0.45; }

  /* Against the button it belongs to rather than in the gap between two, and narrow, so the
     pair reads as one control with a choice attached. */
  .caret {
    display: flex; align-items: center; justify-content: center;
    width: 14px; height: 28px; padding: 0; margin-right: 2px; cursor: pointer;
    font: inherit; background: var(--bg-1); border: 0; border-radius: var(--radius-1);
    color: var(--fg-2);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .caret:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  .caret:disabled { opacity: 0.4; cursor: default; }
</style>
