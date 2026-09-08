<script lang="ts">
  import type { TransferState } from '../state/transfer.svelte';

  /**
   * What is happening on the network, and the way to stop it.
   *
   * Above the panes rather than over them, so it moves nothing the user was reading and covers
   * nothing they might want while they wait. It is the only place in the window that offers to
   * stop something, because fetch, push and clone are the only things that can run long enough
   * to need it.
   */
  const { transfer }: { transfer: TransferState } = $props();
</script>

{#if transfer.current}
  <div class="transfer" role="status" aria-live="polite">
    <span class="what">{transfer.caption}</span>

    <!--
      Determinate once git has counted something, and a moving stripe before that. The two are
      different claims: a bar at zero says "none of it is done", and an unmeasured wait cannot
      say even that.
    -->
    <span class="track" class:waiting={!transfer.measured}>
      {#if transfer.measured}
        <span class="fill" style:width="{transfer.current.percent}%"></span>
      {/if}
    </span>

    {#if transfer.measured}
      <span class="count">
        {transfer.current.current.toLocaleString()} / {transfer.current.total.toLocaleString()}
      </span>
    {/if}

    {#if transfer.waiting > 0}
      <!-- A clone from the start page and a fetch from the toolbar can overlap. Saying how
           many are behind this one is what stops the other looking like it never started. -->
      <span class="queued">+{transfer.waiting} more</span>
    {/if}

    <button class="stop" disabled={transfer.asked} onclick={() => void transfer.cancel()}>
      {transfer.asked ? 'Stopping…' : 'Stop'}
    </button>
  </div>
{/if}

<style>
  .transfer {
    flex: 0 0 auto;
    display: flex; align-items: center; gap: var(--space-3);
    padding: 4px var(--space-3);
    background: var(--bg-1); border-bottom: 1px solid var(--border);
    font-size: var(--text-sm); color: var(--fg-1);
  }
  .what { flex: 0 0 auto; font-weight: 600; }
  .track {
    flex: 1 1 auto; min-width: 0; height: 6px; border-radius: 999px;
    background: var(--bg-3); overflow: hidden;
  }
  .fill { display: block; height: 100%; background: var(--accent); }
  /* A stripe that moves without claiming progress, for the wait before git counts anything. */
  .track.waiting {
    background: linear-gradient(
      90deg, var(--bg-3) 0%, var(--accent-soft) 50%, var(--bg-3) 100%
    );
    background-size: 200% 100%;
    animation: sweep 1.4s linear infinite;
  }
  @keyframes sweep {
    from { background-position: 200% 0; }
    to { background-position: 0 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .track.waiting { animation: none; }
  }
  .count { flex: 0 0 auto; color: var(--fg-2); font-variant-numeric: tabular-nums; }
  .queued { flex: 0 0 auto; color: var(--fg-2); }
  .stop {
    flex: 0 0 auto; font: inherit; font-size: var(--text-sm); cursor: pointer;
    padding: 1px var(--space-2); border-radius: var(--radius-1);
    border: 1px solid var(--border-strong); background: var(--bg-2); color: var(--fg-0);
  }
  .stop:hover:not(:disabled) { background: var(--bg-3); color: var(--fg-0); }
  .stop:disabled { opacity: 0.6; cursor: default; }
</style>
