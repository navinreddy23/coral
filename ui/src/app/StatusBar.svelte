<script lang="ts">
  import Icon from './Icon.svelte';
  import type { Report } from '../state/actions.svelte';
  import type { HostView } from '../ipc/commands';
  import type { PlacedRef } from '../state/refs.svelte';

  const {
    branch,
    head,
    commits,
    changed,
    gitVersion,
    host,
    report,
    busy,
    onDismiss,
    onLogs,
    onHost,
  }: {
    branch: string | null;
    /** The ref for the current branch, for its ahead and behind counts. */
    head: PlacedRef | undefined;
    commits: number;
    changed: number;
    gitVersion: string;
    host: HostView | null;
    report: Report | null;
    busy: boolean;
    onDismiss: () => void;
    /** Opens the activity log, whose way in is the last thing in this bar. */
    onLogs: () => void;
    /** Opens the host's account, which is what the chip saying "no token" is about. */
    onHost: () => void;
  } = $props();

  /** Exact counts stop being useful past a point; the reference caps them at 99+. */
  function cap(n: number): string {
    return n > 99 ? '99+' : String(n);
  }
</script>

<footer class="status" class:busy>
  <span class="branch" title={branch ?? 'detached HEAD'}>
    <span class="glyph"><Icon name="branch" size={12} /></span>{branch ?? 'detached'}
  </span>

  {#if head && (head.ahead > 0 || head.behind > 0)}
    <span class="tally" title="{head.ahead} ahead, {head.behind} behind {head.upstream}">
      <span class="ahead">↑{cap(head.ahead)}</span><span class="behind">↓{cap(head.behind)}</span>
    </span>
  {/if}

  {#if changed > 0}
    <span class="changed">{changed} changed</span>
  {/if}

  <!--
    The one place an action says what happened. It sits between the facts rather than over
    them, so nothing it says can cover a count the user was reading.
  -->
  {#if report}
    <button class="report {report.tone}" onclick={onDismiss} title="Dismiss">
      {report.text}
    </button>
  {:else if busy}
    <span class="working">Working…</span>
  {/if}

  <span class="spacer"></span>

  <!--
    A button, not a label. It said "no token" and there was nowhere in the window to give it
    one; the only way to sign in was the command line.
  -->
  {#if host?.host}
    <button
      class="host"
      onclick={onHost}
      title={`${host.host.owner}/${host.host.repo} at ${host.host.origin}`}
    >
      {host.host.kind === 'gitlab' ? 'GitLab' : 'GitHub'}
      {#if host.token === 'none'}<span class="muted">· no token</span>{/if}
    </button>
  {/if}
  {#if commits > 0}
    <span class="count">{commits.toLocaleString()} commits</span>
  {/if}
  <span class="muted">git {gitVersion}</span>

  <!--
    Last, in the corner of the window, which is where an application's log belongs and where
    the eye goes when something has just happened. It was a button in the title bar, which is
    where the window's own controls are and three rows away from anything it reports on.
  -->
  <button class="logs" onclick={onLogs} title="Activity logs: what Coral has been doing">
    <span class="glyph"><Icon name="logs" size={13} /></span>Logs
  </button>
</footer>

<style>
  .status {
    display: flex; align-items: center; gap: var(--space-3);
    flex: 0 0 auto; height: 24px; padding: 0 var(--space-3);
    border-top: 1px solid var(--border); background: var(--bg-1);
    font-size: var(--text-sm); color: var(--fg-2);
    /* Nothing here may reflow the window: it is the one strip that must stay put while an
       operation runs and the counts either side of it change. */
    overflow: hidden; white-space: nowrap;
  }
  .spacer { flex: 1; }
  .branch { color: var(--fg-1); font-weight: 600; display: inline-flex; align-items: center; gap: 5px; }
  .glyph { color: var(--accent); }
  .tally { display: inline-flex; gap: var(--space-1); }
  .ahead { color: var(--ok); }
  .behind { color: var(--warn); }
  .changed { color: var(--fg-1); }
  .host {
    font: inherit; color: var(--fg-1); cursor: pointer;
    background: none; border: 0; padding: 1px var(--space-2); border-radius: 999px;
  }
  .host:hover { background: var(--bg-2); color: var(--fg-0); }
  .muted { color: var(--fg-2); }

  .report {
    font: inherit; cursor: pointer; border: 0; border-radius: 999px;
    padding: 1px var(--space-2); max-width: 46vw;
    overflow: hidden; text-overflow: ellipsis;
    background: var(--bg-3); color: var(--fg-1);
  }
  .report.warn { background: var(--warn-soft); color: var(--warn); }
  .report.error { background: var(--danger-soft); color: var(--danger); }
  .report.ok { background: var(--ok-soft); color: var(--ok); }

  .logs {
    display: inline-flex; align-items: center; gap: 4px;
    font: inherit; font-size: var(--text-sm); cursor: pointer;
    padding: 1px var(--space-2); margin-right: calc(-1 * var(--space-2));
    border: 0; border-radius: var(--radius-1);
    background: transparent; color: var(--fg-2);
  }
  .logs:hover { background: var(--bg-3); color: var(--fg-0); }

  /* A hairline of accent along the top edge while something is running: visible from the
     corner of the eye, and it moves nothing. */
  .status.busy { border-top-color: var(--accent); }
  .working { color: var(--accent); }
</style>
