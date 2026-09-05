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

  /**
   * Dropping the commit that will not apply, and going on to the next.
   *
   * Only for the operations that replay commits one at a time. `git merge --skip` does not
   * exist: there is one commit being made and skipping it is aborting.
   */
  const skippable = $derived(
    merge.operation !== null &&
      ['rebase', 'cherry_pick', 'revert'].includes(merge.operation.state),
  );

  async function skip() {
    if (await merge.step('skip')) onDone();
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
    {#if skippable}
      <button
        disabled={merge.busy}
        onclick={skip}
        title="Drop the commit that will not apply and go on to the next"
      >
        Skip commit
      </button>
    {/if}
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

        <div class="regions" class:frozen={merge.edited !== null}>
          {#each blocksWithIndex(merge.blocks.blocks) as entry (entry.at)}
            {#if entry.block.kind === 'common'}
              <pre class="common">{entry.block.lines.join('\n')}</pre>
            {:else}
              {@const picked = merge.choices[entry.conflict]}
              <div class="conflict" class:undecided={picked === undefined}>
                {#each sides as { side, label } (side)}
                  {@const lines =
                    side === 'ours'
                      ? entry.block.ours
                      : side === 'theirs'
                        ? entry.block.theirs
                        : entry.block.base}
                  {@const order = picked?.indexOf(side) ?? -1}
                  {#if side !== 'base' || lines.length > 0}
                    <button
                      class="side {side}"
                      class:picked={order >= 0}
                      disabled={merge.edited !== null}
                      title={order >= 0 ? `Taken. Click to leave it out` : 'Take this side'}
                      onclick={() => merge.toggle(entry.conflict, side)}
                    >
                      <span class="who">
                        {label}
                        <!-- Which of the taken sides comes first, when more than one is. -->
                        {#if order >= 0 && (picked?.length ?? 0) > 1}
                          <span class="order">{order + 1}</span>
                        {/if}
                      </span>
                      <pre>{lines.length === 0 ? '(nothing)' : lines.join('\n')}</pre>
                    </button>
                  {/if}
                {/each}
                {#if picked?.length === 0}
                  <span class="neither">this region takes nothing</span>
                {/if}
              </div>
            {/if}
          {/each}
        </div>

        <!-- What is actually going to be written. Two sides picked in order settles most
             conflicts but not all, so the result can also be typed over. -->
        <div class="result">
          <div class="bar">
            <span class="path">Result</span>
            <span class="muted">{merge.output.split('\n').length - 1} lines</span>
            <span class="spacer"></span>
            {#if merge.edited === null}
              <button onclick={() => merge.edit(merge.output)}>Edit it by hand</button>
            {:else}
              <button onclick={() => merge.unedit()}>Back to picking sides</button>
            {/if}
          </div>
          {#if merge.edited === null}
            <pre class="output">{merge.output}</pre>
          {:else}
            <textarea
              class="output"
              spellcheck="false"
              aria-label="The resolved file"
              value={merge.edited}
              oninput={(e) => merge.edit(e.currentTarget.value)}
            ></textarea>
          {/if}
        </div>
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
  .order {
    display: inline-block; min-width: 13px; padding: 0 3px; margin-left: 4px;
    background: var(--accent); color: var(--accent-fg); border-radius: 7px;
    font-size: 9px; text-align: center;
  }
  .neither {
    align-self: center; padding: 0 var(--space-2); background: var(--bg-0);
    font-size: 10px; color: var(--fg-2);
  }
  /* While the result is being typed over, the picks no longer decide it — saying so is
     better than leaving buttons that look live and change nothing. */
  .regions.frozen { opacity: 0.55; }

  .result { border-top: 1px solid var(--border-strong); margin-top: var(--space-3); }
  .result .bar { position: static; }
  .output {
    display: block; width: 100%; box-sizing: border-box; min-height: 120px;
    max-height: 320px; overflow: auto; padding: var(--space-2) var(--space-3);
    background: var(--bg-0); color: var(--fg-0); border: 0;
    font-family: var(--font-mono); font-size: 11px; line-height: 17px;
    white-space: pre; resize: vertical;
  }
  textarea.output { outline: 1px solid var(--accent); outline-offset: -1px; }
  .muted { color: var(--fg-2); padding: var(--space-3); font-size: 12px; }
  .error { color: var(--danger); padding: var(--space-2) var(--space-3); font-size: 12px; margin: 0; }
  .stopped {
    margin: 0; padding: var(--space-2) var(--space-3); font-size: 11px;
    color: var(--warn); background: var(--warn-soft);
    border-bottom: 1px solid var(--border); white-space: pre-wrap;
  }
</style>
