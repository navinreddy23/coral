<script lang="ts">
  import type { BisectState } from '../state/bisect.svelte';
  import type { Verdict } from '../ipc/types';

  let {
    bisect,
    busy,
    subjectOf,
    onMark,
    onEnd,
    onShow,
  }: {
    bisect: BisectState;
    busy: boolean;
    /** The summary of a commit, when the loaded rows hold it. */
    subjectOf: (oid: string) => string | null;
    onMark: (verdict: Verdict) => void;
    onEnd: () => void;
    onShow: (oid: string) => void;
  } = $props();

  const b = $derived(bisect.current);
  const short = (oid: string) => oid.slice(0, 8);
  const cap = (word: string) => word.charAt(0).toUpperCase() + word.slice(1);
</script>

{#if b !== null}
  <div class="bisect" role="status">
    <span class="what">Bisecting</span>
    {#if b.found !== null}
      {@const subject = subjectOf(b.found)}
      <span class="news">
        First {b.terms.bad} commit:
        <button class="link" onclick={() => onShow(b.found ?? '')}>{short(b.found)}</button>
        {#if subject !== null}<span class="subject">{subject}</span>{/if}
      </span>
    {:else if b.mistaken}
      <span class="news">
        A commit marked {b.terms.good} is newer than the one marked {b.terms.bad}, as if the two
        were swapped. End the bisect and start again.
      </span>
    {:else if b.onlySkippedLeft}
      <span class="news">Only skipped commits are left; git cannot narrow it further.</span>
    {:else if b.bad === null}
      <span class="news">Mark a commit that is {b.terms.bad}: right-click it in the list.</span>
    {:else if b.good.length === 0}
      <span class="news">Now mark one that was {b.terms.good}: right-click it in the list.</span>
    {:else if b.testing !== null}
      <span class="news">
        Testing
        <button class="link" onclick={() => onShow(b.testing ?? '')}>{short(b.testing)}</button>
        {#if b.steps !== null}
          · about {b.steps} more {b.steps === 1 ? 'step' : 'steps'}, {b.suspects} suspects
        {/if}
      </span>
      <span class="verdicts">
        <button disabled={busy} onclick={() => onMark('good')}>{cap(bisect.word('good'))}</button>
        <button disabled={busy} onclick={() => onMark('bad')}>{cap(bisect.word('bad'))}</button>
        <button disabled={busy} onclick={() => onMark('skip')}>Skip</button>
      </span>
    {/if}
    <button class="end" disabled={busy} onclick={onEnd}>End bisect</button>
  </div>
{/if}

<style>
  .bisect {
    position: relative; z-index: 5;
    display: flex; align-items: center; gap: var(--space-3); flex-wrap: wrap;
    padding: var(--space-2) var(--space-4);
    background: var(--accent-soft); color: var(--fg-0); font-size: var(--text-base);
  }
  .what { font-weight: var(--weight-semibold); color: var(--accent); }
  .news { display: inline-flex; align-items: baseline; gap: var(--space-2); min-width: 0; }
  .subject { color: var(--fg-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .verdicts { display: inline-flex; gap: var(--space-2); }
  .end { margin-left: auto; }
  button {
    font: inherit; cursor: pointer; padding: 1px var(--space-3);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border); border-radius: var(--radius-1);
  }
  button:hover:not(:disabled) { background: var(--bg-2); }
  button:disabled { color: var(--fg-2); cursor: default; }
  .link {
    padding: 0; border: none; background: none; color: var(--accent);
    font-family: var(--font-mono); text-decoration: underline;
  }
  .link:hover:not(:disabled) { background: none; }
</style>
