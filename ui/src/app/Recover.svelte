<script lang="ts">
  import Icon from './Icon.svelte';
  import { shortAge } from './age';
  import type { ReflogState } from '../state/reflog.svelte';
  import type { ReflogEntry } from '../ipc/types';

  const {
    reflog,
    onBranch,
    onCheckout,
    onClose,
  }: {
    reflog: ReflogState;
    /** Asks for a name and makes a branch at the commit, which is what keeps it. Stays put. */
    onBranch: (oid: string) => void;
    onCheckout: (oid: string) => void;
    onClose: () => void;
  } = $props();

  function when(seconds: number): string {
    return new Date(seconds * 1000).toLocaleString();
  }

  /** git's words for the move, less the subject a commit or amend repeats from the line above. */
  function said(entry: ReflogEntry): string {
    const repeated = `: ${entry.summary}`;
    return entry.action.endsWith(repeated) ? entry.action.slice(0, -repeated.length) : entry.action;
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<div class="sheet">
  <header>
    <h2>Recover lost work</h2>
    <button class="shut" title="Close" onclick={onClose}><Icon name="close" size={15} /></button>
  </header>

  <nav class="bar">
    <p class="lede">
      Every commit HEAD has been on, newest first. One marked <span class="lost-word">lost</span>
      is on no branch, tag or stash any more; make a branch at it to keep it.
    </p>
    <label class="only">
      <input type="checkbox" bind:checked={reflog.onlyLost} />
      Only lost ({reflog.lostCount})
    </label>
    <button class="act" onclick={() => void reflog.reload()}>Refresh</button>
  </nav>

  {#if reflog.error}
    <p class="note error">{reflog.error}</p>
  {:else if reflog.loading && reflog.entries.length === 0}
    <p class="note">Reading where HEAD has been…</p>
  {:else if reflog.shown.length === 0}
    <p class="note">
      {reflog.onlyLost
        ? 'Nothing is lost: every commit HEAD has been on is still on a branch, tag or stash.'
        : 'HEAD has not moved yet.'}
    </p>
  {:else}
    <ol class="rows">
      {#each reflog.shown as entry (entry.index)}
        <li class:lost={entry.lost}>
          <span class="age" title={when(entry.time)}>{shortAge(entry.time)}</span>
          <code class="oid" title={entry.oid}>{entry.oid.slice(0, 8)}</code>
          <span class="text">
            <span class="summary">{entry.summary}</span>
            <span class="action">{said(entry)}</span>
          </span>
          {#if entry.lost}<span class="badge">lost</span>{:else}<span></span>{/if}
          <span class="buttons">
            <button class="act" title="Make a branch at this commit, without leaving the one you are on" onclick={() => onBranch(entry.oid)}>
              <Icon name="branch" size={12} /> Branch here…
            </button>
            <button class="act" title="Check this commit out, detached" onclick={() => onCheckout(entry.oid)}>
              Check out
            </button>
          </span>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .sheet {
    position: absolute; inset: 0; z-index: 30;
    display: flex; flex-direction: column;
    background: var(--bg-1); color: var(--fg-0);
  }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
  }
  h2 { margin: 0; font-size: var(--text-lg); font-weight: var(--weight-semibold); flex: 1; }
  .shut {
    display: flex; font: inherit; line-height: 1; cursor: pointer;
    background: var(--bg-1); border: 0; color: var(--fg-2); padding: 0 var(--space-2);
  }
  .shut:hover { color: var(--fg-0); }

  .bar {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-2) var(--space-4); border-bottom: 1px solid var(--border);
  }
  .lede { margin: 0; flex: 1; color: var(--fg-2); font-size: var(--text-sm); }
  .lost-word { color: var(--warn); font-weight: var(--weight-semibold); }
  .only {
    display: flex; align-items: center; gap: var(--space-1);
    font-size: var(--text-sm); color: var(--fg-1); white-space: nowrap;
  }
  .act {
    display: inline-flex; align-items: center; gap: var(--space-1);
    font: inherit; font-size: var(--text-sm); cursor: pointer; white-space: nowrap;
    padding: 2px var(--space-3);
    background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    color: var(--fg-1);
  }
  .act:hover { background: var(--bg-3); color: var(--fg-0); }

  .note { margin: 0; padding: var(--space-4); color: var(--fg-2); font-size: var(--text-base); }
  .note.error { color: var(--danger); }

  .rows {
    list-style: none; margin: 0; padding: var(--space-2) var(--space-4);
    flex: 1; min-height: 0; overflow-y: auto;
  }
  /* Fixed columns, so the ages and ids line up and the lost ones can be seen down the side. */
  .rows li {
    display: grid; grid-template-columns: 3.5em 6em minmax(0, 1fr) 3.5em auto;
    align-items: center; gap: var(--space-3);
    padding: var(--space-1) 0; border-bottom: 1px solid var(--border);
    background: var(--bg-1); color: var(--fg-1); font-size: var(--text-sm);
  }
  .age { color: var(--fg-2); font-variant-numeric: tabular-nums; }
  .oid { font-family: var(--font-mono); color: var(--fg-2); }
  .text { display: flex; flex-direction: column; min-width: 0; }
  .summary, .action { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .summary { color: var(--fg-0); }
  .action { color: var(--fg-2); font-size: var(--text-xs); }
  .badge {
    justify-self: start; padding: 0 var(--space-2); border-radius: var(--radius-1);
    font-size: var(--text-xs); font-weight: var(--weight-semibold);
    color: var(--warn); border: 1px solid var(--warn);
  }
  .buttons { display: flex; gap: var(--space-2); }
  /* Out of the way until wanted on a row nothing is lost from: those are rarely what is sought. */
  .rows li:not(.lost) .buttons { opacity: 0; }
  .rows li:hover .buttons, .rows li:focus-within .buttons { opacity: 1; }
</style>
