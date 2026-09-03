<script lang="ts">
  import type { Step } from '../ipc/types';
  import type { RebaseState } from '../state/rebase.svelte';

  const { rebase, onDone }: { rebase: RebaseState; onDone: () => void } = $props();

  /**
   * Reword is absent deliberately.
   *
   * It needs a new message per commit, which means an editor git opens mid-rebase and nothing
   * here can answer. Editing a message is `edit`, which stops for it, or an amend afterwards.
   */
  const steps: { step: Step; hint: string }[] = [
    { step: 'pick', hint: 'Keep the commit as it is' },
    { step: 'edit', hint: 'Stop here so the commit can be changed' },
    { step: 'squash', hint: 'Fold into the one above, keeping both messages' },
    { step: 'fixup', hint: 'Fold into the one above, discarding this message' },
    { step: 'drop', hint: 'Leave the commit out' },
  ];

  let dragging = $state<number | null>(null);

  async function run() {
    if (await rebase.start()) onDone();
  }
</script>

<div class="scrim" role="presentation" onclick={() => rebase.close()}>
  <div class="panel" role="presentation" onclick={(e) => e.stopPropagation()}>
    <header>
      <span>Rebase onto <strong>{rebase.onto}</strong></span>
      <span class="muted">
        {rebase.remaining} of {rebase.items.length} commits kept, oldest first
      </span>
    </header>

    {#if rebase.error}
      <p class="error">{rebase.error}</p>
    {/if}

    {#if rebase.loading}
      <p class="muted pad">Reading the range…</p>
    {:else}
      <ul>
        {#each rebase.items as item, i (item.oid)}
          <li
            class:dropped={item.step === 'drop'}
            class:over={dragging !== null && dragging !== i}
            draggable="true"
            ondragstart={() => (dragging = i)}
            ondragend={() => (dragging = null)}
            ondragover={(e) => e.preventDefault()}
            ondrop={(e) => {
              e.preventDefault();
              if (dragging !== null) rebase.move(dragging, i);
              dragging = null;
            }}
          >
            <span class="grip" aria-hidden="true">⠿</span>
            <select
              value={item.step}
              aria-label="What to do with this commit"
              onchange={(e) => rebase.setStep(i, e.currentTarget.value as Step)}
            >
              {#each steps as s (s.step)}
                <option value={s.step} title={s.hint}>{s.step}</option>
              {/each}
            </select>
            <span class="sha mono">{item.oid.slice(0, 8)}</span>
            <span class="summary">{item.summary}</span>
          </li>
        {/each}
      </ul>
    {/if}

    <footer>
      {#if rebase.invalid}
        <span class="warn">
          The first commit has nothing above it to fold into.
        </span>
      {/if}
      <span class="spacer"></span>
      <button onclick={() => rebase.close()}>Cancel</button>
      <button
        class="primary"
        disabled={rebase.busy || rebase.invalid || rebase.items.length === 0}
        onclick={run}
      >
        Start rebase
      </button>
    </footer>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 20; background: rgb(0 0 0 / 28%);
    display: flex; justify-content: center; align-items: flex-start; padding-top: 8vh;
  }
  .panel {
    width: min(720px, 92vw); max-height: 76vh; display: flex; flex-direction: column;
    background: var(--bg-0); border: 1px solid var(--border); border-radius: 6px;
    overflow: hidden; font-size: 12px;
  }
  header {
    display: flex; align-items: baseline; gap: var(--space-3);
    padding: var(--space-3); border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  ul { list-style: none; margin: 0; padding: var(--space-1) 0; overflow-y: auto; flex: 1; }
  li {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 2px var(--space-3);
  }
  li:hover { background: var(--bg-1); }
  li.over { outline: 1px dashed var(--accent); outline-offset: -1px; }
  /* A dropped commit stays in place, struck through, so its position is still readable. */
  .dropped .summary, .dropped .sha { text-decoration: line-through; color: var(--fg-2); }
  .grip { cursor: grab; color: var(--fg-2); user-select: none; }
  select {
    font: inherit; font-size: 11px; width: 6.5em;
    background: var(--bg-0); color: var(--fg-1);
    border: 1px solid var(--border); border-radius: 3px;
  }
  .sha { color: var(--fg-2); flex: 0 0 auto; }
  .summary { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  footer {
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-top: 1px solid var(--border); background: var(--bg-1);
  }
  .spacer { flex: 1; }
  button {
    font: inherit; font-size: 11px; cursor: pointer; padding: 2px var(--space-3);
    background: var(--bg-0); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1);
  }
  button:disabled { opacity: 0.5; cursor: default; }
  button.primary:not(:disabled) {
    background: var(--accent); color: #fff; border-color: var(--accent);
  }
  .warn { color: var(--danger); }
  .muted { color: var(--fg-2); }
  .pad { padding: var(--space-3); }
  .error { color: var(--danger); margin: 0; padding: var(--space-2) var(--space-3); }
</style>
