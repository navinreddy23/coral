<script lang="ts">
  import type { ToastsState } from '../state/toasts.svelte';

  const { toasts }: { toasts: ToastsState } = $props();

  const GLYPH: Record<string, string> = {
    ok: '✓',
    info: 'i',
    warn: '!',
    error: '×',
  };
</script>

<!--
  Stacked bottom-up in the corner, newest nearest the edge, each closable on its own. A fetch of
  three remotes has three things to say and the second must not overwrite the first.

  The left corner, not the right: a warning waits to be dismissed rather than timing out, and
  on the right it sat over the commit message and the Commit button for as long as it waited.
  Nothing in the window puts a control in the bottom left.
-->
{#if toasts.items.length > 0}
  <div class="stack" role="status" aria-live="polite">
    {#each toasts.items as toast (toast.id)}
      <div class="toast {toast.kind}">
        <span class="mark" aria-hidden="true">{GLYPH[toast.kind] ?? '·'}</span>
        <div class="text">
          <p class="title">{toast.title}</p>
          {#if toast.detail}<p class="detail">{toast.detail}</p>{/if}
        </div>
        <button class="shut" onclick={() => toasts.dismiss(toast.id)} title="Dismiss">×</button>
      </div>
    {/each}
    {#if toasts.items.length > 1}
      <button class="all" onclick={() => toasts.clear()}>Dismiss all</button>
    {/if}
  </div>
{/if}

<style>
  .stack {
    position: fixed; left: var(--space-4); bottom: var(--space-5); z-index: 50;
    display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-2);
    max-width: min(30em, calc(100vw - 2 * var(--space-4)));
  }
  .toast {
    display: flex; align-items: flex-start; gap: var(--space-2);
    width: 100%; box-sizing: border-box;
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-2);
    /* It floats over the window, so it needs to read as above the page rather than in it. */
    box-shadow: 0 6px 20px rgb(0 0 0 / 16%);
    font-size: 12px;
  }
  /* The kind is carried by a bar down the leading edge as well as by the glyph, so it survives
     being read at a glance and by anyone who cannot separate the colours. */
  .toast.ok { box-shadow: inset 3px 0 0 var(--ok), 0 6px 20px rgb(0 0 0 / 16%); }
  .toast.info { box-shadow: inset 3px 0 0 var(--accent), 0 6px 20px rgb(0 0 0 / 16%); }
  .toast.warn { box-shadow: inset 3px 0 0 var(--warn), 0 6px 20px rgb(0 0 0 / 16%); }
  .toast.error { box-shadow: inset 3px 0 0 var(--danger), 0 6px 20px rgb(0 0 0 / 16%); }

  .mark {
    flex: 0 0 auto; width: 16px; height: 16px; line-height: 16px; text-align: center;
    border-radius: 50%; font-size: 11px; font-weight: 700; color: var(--accent-fg);
  }
  .toast.ok .mark { background: var(--ok); }
  .toast.info .mark { background: var(--accent); }
  .toast.warn .mark { background: var(--warn); }
  .toast.error .mark { background: var(--danger); }

  .text { flex: 1; min-width: 0; }
  .title { margin: 0; font-weight: 600; }
  .detail {
    margin: 2px 0 0; color: var(--fg-1); font-size: 11px; line-height: 1.4;
    /* git can report a paragraph; the toast shows the first few lines and no more. */
    max-height: 4.5em; overflow: hidden; white-space: pre-wrap; word-break: break-word;
  }
  .shut {
    flex: 0 0 auto; font: inherit; font-size: 14px; line-height: 1; cursor: pointer;
    padding: 0 var(--space-1); background: none; border: 0; color: var(--fg-2);
  }
  .shut:hover { color: var(--fg-0); }
  .all {
    font: inherit; font-size: 11px; cursor: pointer;
    padding: 2px var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .all:hover { background: var(--bg-3); color: var(--fg-0); }
</style>
