<script lang="ts">
  import { elidePath } from './path';
  import type { MergeState, Side } from '../state/merge.svelte';

  const { merge, onDone }: { merge: MergeState; onDone: () => void } = $props();

  /**
   * Which side is which.
   *
   * A rebase replays your commits onto theirs, so "ours" is the branch being rebased onto and
   * "theirs" is your own work — backwards from what everyone expects. The engine reports that;
   * saying it here once is cheaper than leaving people to work it out mid-conflict.
   */
  const labels = $derived(merge.operation?.labels ?? { ours: 'ours', theirs: 'theirs', swapped: false });

  const sides: { side: Side; label: string }[] = $derived([
    { side: 'ours', label: labels.ours },
    { side: 'theirs', label: labels.theirs },
    { side: 'base', label: 'base' },
  ]);

  async function finish() {
    if (await merge.step('continue')) onDone();
  }

  async function abort() {
    await merge.step('abort');
    onDone();
  }
</script>

<section class="merge">
  <header>
    <span class="what">
      {merge.operation?.state ?? 'merge'} in progress
      {#if merge.operation?.progress}
        <span class="muted">
          — {merge.operation.progress.current} of {merge.operation.progress.total}
        </span>
      {/if}
    </span>
    {#if labels.swapped}
      <span class="warn" title="A rebase replays your commits onto the other branch">
        sides are reversed during a rebase
      </span>
    {/if}
    <span class="spacer"></span>
    <button
      class="primary"
      disabled={merge.busy || merge.files.length > 0}
      onclick={finish}
      title={merge.files.length > 0 ? 'Resolve every file first' : 'Continue the operation'}
    >
      Continue
    </button>
    <button disabled={merge.busy} onclick={abort}>Abort</button>
  </header>

  {#if merge.error}
    <p class="error">{merge.error}</p>
  {/if}
  <!-- A rebase stops once per conflicting commit, so continuing usually lands on the next one.
       Saying which commit failed to apply is the difference between that and a file list that
       has quietly refilled. -->
  {#if merge.stopped !== ''}
    <p class="stopped mono">{merge.stopped}</p>
  {/if}

  <div class="body">
    <ul class="files">
      {#each merge.files as file (file.path)}
        {@const blockwise = !file.binary && !file.deleteModify}
        <li>
          <button
            class="file"
            class:on={file.path === merge.active}
            disabled={!blockwise}
            title={blockwise
              ? file.path
              : `${file.path} — binary or deleted on one side, so only a whole-file choice applies`}
            onclick={() => merge.open(file.path)}
          >
            <span class="name">{elidePath(file.path, 40)}</span>
            {#if !blockwise}<span class="tag">whole file</span>{/if}
          </button>
          <span class="wholesale">
            <button disabled={merge.busy} onclick={() => merge.take(file.path, { kind: 'ours' })}>
              {labels.ours}
            </button>
            <button disabled={merge.busy} onclick={() => merge.take(file.path, { kind: 'theirs' })}>
              {labels.theirs}
            </button>
            {#if file.deleteModify}
              <button disabled={merge.busy} onclick={() => merge.take(file.path, { kind: 'delete' })}>
                delete
              </button>
            {/if}
          </span>
        </li>
      {/each}
      {#if merge.files.length === 0}
        <li class="done">Every file is resolved. Continue when you are ready.</li>
      {/if}
    </ul>

    <div class="blocks">
      {#if merge.active === null}
        <p class="muted">Pick a file to resolve it region by region.</p>
      {:else if merge.blocks === null}
        <p class="muted">Loading…</p>
      {:else}
        <div class="bar">
          <span class="path mono">{merge.active}</span>
          <span class="muted">
            {merge.conflicts.length} region{merge.conflicts.length === 1 ? '' : 's'}
          </span>
          <span class="spacer"></span>
          <button onclick={() => merge.chooseAll('ours')}>All {labels.ours}</button>
          <button onclick={() => merge.chooseAll('theirs')}>All {labels.theirs}</button>
          <button class="primary" disabled={merge.busy || !merge.settled} onclick={() => merge.apply()}>
            Mark resolved
          </button>
        </div>

        {#each blocksWithIndex(merge.blocks.blocks) as entry (entry.at)}
          {#if entry.block.kind === 'common'}
            <pre class="common">{entry.block.lines.join('\n')}</pre>
          {:else}
            {@const chosen = merge.choices[entry.conflict]}
            <div class="conflict" class:undecided={chosen === undefined}>
              {#each sides as { side, label } (side)}
                {@const lines =
                  side === 'ours'
                    ? entry.block.ours
                    : side === 'theirs'
                      ? entry.block.theirs
                      : entry.block.base}
                {#if side !== 'base' || lines.length > 0}
                  <button
                    class="side {side}"
                    class:picked={chosen === side}
                    onclick={() => merge.choose(entry.conflict, side)}
                  >
                    <span class="who">{label}</span>
                    <pre>{lines.length === 0 ? '(nothing)' : lines.join('\n')}</pre>
                  </button>
                {/if}
              {/each}
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  </div>
</section>

<script module lang="ts">
  import type { Block } from '../ipc/types';

  /** Blocks paired with their index among the conflicts, which is how a decision is keyed. */
  export function blocksWithIndex(
    blocks: readonly Block[],
  ): { at: number; block: Block; conflict: number }[] {
    let conflict = 0;
    return blocks.map((block, at) => ({
      at,
      block,
      conflict: block.kind === 'conflict' ? conflict++ : -1,
    }));
  }
</script>

<style>
  .merge { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--bg-0); }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    height: 34px; padding: 0 var(--space-3); flex: 0 0 auto;
    border-bottom: 1px solid var(--border); background: var(--bg-1); font-size: 12px;
  }
  .spacer { flex: 1; }
  .warn { color: var(--danger); font-size: 11px; }
  button {
    font: inherit; font-size: 11px; cursor: pointer; padding: 2px var(--space-2);
    background: var(--bg-0); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1);
  }
  button:disabled { opacity: 0.5; cursor: default; }
  button.primary:not(:disabled) { background: var(--accent); color: #fff; border-color: var(--accent); }

  .body { flex: 1; display: flex; min-height: 0; }
  .files {
    width: 260px; flex: 0 0 auto; list-style: none; margin: 0; padding: var(--space-2) 0;
    overflow-y: auto; border-right: 1px solid var(--border); background: var(--bg-1);
  }
  /* The path on its own line and the wholesale choices beneath it: side by side, two branch
     names left the path about eighty pixels, which is not enough to tell two files apart. */
  .files li {
    display: flex; flex-direction: column; align-items: stretch; gap: 2px;
    padding: var(--space-1) var(--space-2);
  }
  .files li + li { border-top: 1px solid var(--border); }
  .file {
    flex: 1; min-width: 0; text-align: left; border: 0; background: var(--bg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .file.on { background: var(--accent-soft); color: var(--fg-0); }
  .tag { font-size: 10px; color: var(--fg-2); }
  .wholesale { display: flex; gap: 2px; flex: 0 0 auto; padding-left: var(--space-3); }
  .wholesale button { font-size: 10px; padding: 0 var(--space-2); }
  .done { padding: var(--space-3); font-size: 12px; color: var(--fg-2); }

  .blocks { flex: 1; min-width: 0; overflow: auto; padding-bottom: var(--space-4); }
  .bar {
    position: sticky; top: 0; z-index: 1;
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-2) var(--space-3); background: var(--bg-1);
    border-bottom: 1px solid var(--border); font-size: 12px;
  }
  .path { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  pre {
    margin: 0; white-space: pre; font-family: var(--font-mono); font-size: 11px;
    line-height: 17px; overflow-x: auto;
  }
  .common { padding: 0 var(--space-3); color: var(--fg-1); }
  .conflict { display: flex; gap: 1px; margin: var(--space-2) 0; background: var(--border); }
  /* Undecided regions are outlined so an unresolved one is visible while scrolling past. */
  .conflict.undecided { outline: 1px solid var(--danger); }
  .side {
    flex: 1; min-width: 0; text-align: left; border: 0; border-radius: 0;
    padding: var(--space-1) var(--space-2); background: var(--bg-0);
  }
  .side.ours { background: var(--add-bg); }
  .side.theirs { background: var(--remove-bg); }
  .side.base { background: var(--bg-2); }
  .side.picked { outline: 2px solid var(--accent); outline-offset: -2px; }
  .who {
    display: block; font-size: 10px; text-transform: uppercase; letter-spacing: 0.04em;
    color: var(--fg-2); margin-bottom: 2px;
  }
  .muted { color: var(--fg-2); padding: var(--space-3); font-size: 12px; }
  .error { color: var(--danger); padding: var(--space-2) var(--space-3); font-size: 12px; margin: 0; }
  .stopped {
    margin: 0; padding: var(--space-2) var(--space-3); font-size: 11px;
    color: var(--warn); background: var(--warn-soft);
    border-bottom: 1px solid var(--border); white-space: pre-wrap;
  }
</style>
