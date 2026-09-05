<script lang="ts">
  import { elidePath } from './path';
  import type { ConflictedFile } from '../ipc/types';
  import type { MergeState } from '../state/merge.svelte';

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

  /** The two sides, in the order the panes draw them. */
  const panes = $derived([
    { side: 'ours' as const, label: labels.ours, tag: 'A' },
    { side: 'theirs' as const, label: labels.theirs, tag: 'B' },
  ]);

  /** Which conflict the stepper is on. Reset whenever another file is opened. */
  let at = $state(0);
  $effect(() => {
    void merge.active;
    at = 0;
  });

  /**
   * Moves to another conflict and brings it into view in all three panes.
   *
   * A file with a dozen conflicts is a file where scrolling three panes by hand is the work,
   * which is why every merge tool has this pair of arrows.
   */
  function step(by: number) {
    const total = merge.conflicts.length;
    if (total === 0) return;
    at = (at + by + total) % total;
    for (const id of [`ours-conflict-${at}`, `theirs-conflict-${at}`, `out-conflict-${at}`]) {
      document.getElementById(id)?.scrollIntoView({ block: 'center' });
    }
  }

  /** What to call the operation. The state is an enum name; `cherry_pick` is not a word. */
  const verb = $derived.by(() => {
    const state = merge.operation?.state ?? 'merge';
    if (state === 'cherry_pick') return 'cherry-pick';
    return state === 'clean' || state === 'bisect' ? 'merge' : state;
  });

  /**
   * Which sides a conflict actually has content on.
   *
   * A file deleted on one side and modified on the other has one version, not two, and
   * offering the missing one gave a button that could only ever fail. git's own names for
   * these say which side is gone.
   */
  function sidesOf(file: ConflictedFile): { ours: boolean; theirs: boolean } {
    const kind = file.kind;
    return {
      ours: !['added_by_them', 'deleted_by_us', 'both_deleted'].includes(kind),
      theirs: !['added_by_us', 'deleted_by_them', 'both_deleted'].includes(kind),
    };
  }

  /** The file being looked at, when it is one that cannot be picked apart. */
  const wholeFile = $derived(
    merge.files.find((f) => f.path === merge.active && !supportsBlocks(f)) ?? null,
  );

  function supportsBlocks(file: ConflictedFile): boolean {
    return !file.binary && !file.deleteModify;
  }

  /** Why this file cannot be settled region by region, in words rather than a flag. */
  function whyWhole(file: ConflictedFile): string {
    if (file.binary) return `${file.path} is binary, so there are no lines to pick between.`;
    const gone = sidesOf(file);
    if (!gone.ours) {
      return `${file.path} is not on ${labels.ours} at all: ${labels.theirs} changed a file this side had deleted.`;
    }
    if (!gone.theirs) {
      return `${file.path} was deleted by ${labels.theirs}, and changed on ${labels.ours}.`;
    }
    return `${file.path} was deleted on both sides.`;
  }
</script>

<section class="merge">
  <header>
    <span class="what">
      {verb} in progress
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
        {@const blockwise = supportsBlocks(file)}
        {@const has = sidesOf(file)}
        <li>
          <!-- Selectable whether or not it can be picked apart. A file with one version left
               still has a decision to make, and disabling it left the pane telling people to
               pick regions in a file that has none. -->
          <button
            class="file"
            class:on={file.path === merge.active}
            title={file.path}
            onclick={() => merge.open(file.path)}
          >
            <span class="name">{elidePath(file.path, 40)}</span>
            {#if !blockwise}<span class="tag">whole file</span>{/if}
          </button>
          <span class="wholesale">
            {#if has.ours}
              <button
                disabled={merge.busy}
                title="Take the version on {labels.ours}"
                onclick={() => merge.take(file.path, { kind: 'ours' })}
              >
                {labels.ours}
              </button>
            {/if}
            {#if has.theirs}
              <button
                disabled={merge.busy}
                title="Take the version from {labels.theirs}"
                onclick={() => merge.take(file.path, { kind: 'theirs' })}
              >
                {labels.theirs}
              </button>
            {/if}
            {#if file.deleteModify}
              <button
                disabled={merge.busy}
                title="Leave the file deleted"
                onclick={() => merge.take(file.path, { kind: 'delete' })}
              >
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
        <p class="muted">
          {merge.files.length === 0
            ? 'Nothing is left conflicted.'
            : 'Pick a file to settle it.'}
        </p>
      {:else if wholeFile}
        <!-- One version of the file exists, or none, so there is nothing to pick between
             line by line. Saying which choice is on offer beats a pane that asks for regions
             the file does not have. -->
        <div class="whole">
          <p>{whyWhole(wholeFile)}</p>
          <div class="choices">
            {#if sidesOf(wholeFile).ours}
              <button
                class="primary"
                disabled={merge.busy}
                onclick={() => merge.take(wholeFile.path, { kind: 'ours' })}
              >
                Keep what is on {labels.ours}
              </button>
            {/if}
            {#if sidesOf(wholeFile).theirs}
              <button
                class="primary"
                disabled={merge.busy}
                onclick={() => merge.take(wholeFile.path, { kind: 'theirs' })}
              >
                Take the version from {labels.theirs}
              </button>
            {/if}
            <!-- Only where a side actually deleted it. A binary file both sides changed has
                 no deletion in it, and offering one is offering a third answer to a question
                 with two. -->
            {#if wholeFile.deleteModify}
              <button
                disabled={merge.busy}
                onclick={() => merge.take(wholeFile.path, { kind: 'delete' })}
              >
                Leave it deleted
              </button>
            {/if}
          </div>
        </div>
      {:else if merge.blocks === null}
        <p class="muted">Loading…</p>
      {:else}
        <div class="bar">
          <span class="path mono">{merge.active}</span>
          <span class="muted">
            {merge.conflicts.length} conflict{merge.conflicts.length === 1 ? '' : 's'}{merge.untouched >
            0
              ? `, ${merge.untouched} untouched`
              : ''}
          </span>
          <span class="spacer"></span>
          <button onclick={() => merge.chooseAll('ours')}>All {labels.ours}</button>
          <button onclick={() => merge.chooseAll('theirs')}>All {labels.theirs}</button>
          <button class="primary" disabled={merge.busy} onclick={() => merge.apply()}>
            Mark resolved
          </button>
        </div>

        <!-- The two sides above, the result below, as every merge tool worth using lays it
             out: the whole file on each side rather than the conflicting lines alone, because
             which side to take is a question about the code around them. -->
        <div class="sheets" class:frozen={merge.edited !== null}>
          {#each panes as pane (pane.side)}
            <div class="pane {pane.side}">
              <div class="head">
                <span class="tag">{pane.tag}</span>
                <span class="who">{pane.label}</span>
                <span class="spacer"></span>
                <button
                  disabled={merge.edited !== null}
                  title="Take every line of this side, for conflict {at + 1}"
                  onclick={() => merge.toggle(at, pane.side)}
                >
                  Take all
                </button>
              </div>
              <div class="code">
                {#each sideRows(merge.blocks.blocks, pane.side) as row (row.at)}
                  {@const taken =
                    row.conflict !== null &&
                    row.line !== null &&
                    (merge.choices[row.conflict] ?? []).some(
                      (t) => t.side === pane.side && t.line === row.line,
                    )}
                  <div
                    class="line"
                    class:in-conflict={row.conflict !== null}
                    class:taken
                    id={row.first && row.conflict !== null
                      ? `${pane.side}-conflict-${row.conflict}`
                      : undefined}
                  >
                    <span class="tick">
                      {#if row.conflict !== null && row.line !== null}
                        {@const conflict = row.conflict}
                        {@const line = row.line}
                        <input
                          type="checkbox"
                          checked={taken}
                          disabled={merge.edited !== null}
                          aria-label="Take line {row.no} of {pane.label}"
                          onchange={() => merge.toggleLine(conflict, pane.side, line)}
                        />
                      {/if}
                    </span>
                    <span class="no">{row.no}</span>
                    <span class="text">{row.text}</span>
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        </div>

        <div class="output">
          <div class="bar">
            <span class="path">Output</span>
            <span class="spacer"></span>
            {#if merge.conflicts.length > 0}
              <span class="muted">conflict {at + 1} of {merge.conflicts.length}</span>
              <button aria-label="Previous conflict" title="Previous conflict" onclick={() => step(-1)}>
                ↑
              </button>
              <button aria-label="Next conflict" title="Next conflict" onclick={() => step(1)}>
                ↓
              </button>
            {/if}
            {#if merge.edited === null}
              <button onclick={() => merge.edit(merge.output)}>Edit it by hand</button>
            {:else}
              <button onclick={() => merge.unedit()}>Back to picking sides</button>
            {/if}
          </div>
          {#if merge.edited === null}
            <div class="code result">
              {#each outputRows(merge.blocks.blocks, merge.choices) as row (row.at)}
                <div
                  class="line"
                  class:in-conflict={row.conflict !== null}
                  class:here={row.conflict === at}
                  id={row.first && row.conflict !== null ? `out-conflict-${row.conflict}` : undefined}
                >
                  <span class="no">{row.no}</span>
                  <span class="text">{row.text}</span>
                </div>
              {/each}
              {#if outputRows(merge.blocks.blocks, merge.choices).length === 0}
                <p class="muted">The file comes out empty.</p>
              {/if}
            </div>
          {:else}
            <textarea
              class="typed"
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
  import type { Pick, Side } from '../state/merge.svelte';
  import { sideOf } from '../state/merge.svelte';

  /** One line of a file as a pane draws it. */
  export interface Row {
    /** Position in the pane, which is what keys the list. */
    at: number;
    /** Line number in this version of the file, or null for a placeholder. */
    no: number | null;
    text: string;
    /** Which conflict the line belongs to, or null for a line both sides agree on. */
    conflict: number | null;
    /** Position within that side of the conflict, which is what a pick names. */
    line: number | null;
    /** True for the first line of a conflict, which is what the stepper scrolls to. */
    first: boolean;
  }

  /** The file as one side has it: every agreed line, plus that side of every conflict. */
  export function sideRows(blocks: readonly Block[], side: Side): Row[] {
    const out: Row[] = [];
    let conflict = 0;
    for (const block of blocks) {
      if (block.kind === 'common') {
        for (const text of block.lines) push(out, text, null, null, false);
        continue;
      }
      const lines = sideOf(block, side);
      lines.forEach((text, i) => push(out, text, conflict, i, i === 0));
      // A side that adds nothing here still needs a row, or the conflict has no place in
      // this pane at all and the stepper has nothing to scroll to.
      if (lines.length === 0) push(out, '(nothing on this side)', conflict, null, true, false);
      conflict += 1;
    }
    return out;
  }

  /** The file as it will be written, with each line tied back to the conflict it came from. */
  export function outputRows(blocks: readonly Block[], choices: Record<number, Pick>): Row[] {
    const out: Row[] = [];
    let conflict = 0;
    for (const block of blocks) {
      if (block.kind === 'common') {
        for (const text of block.lines) push(out, text, null, null, false);
        continue;
      }
      const pick = choices[conflict] ?? [];
      const lines =
        pick.length === 0
          ? [...block.base]
          : pick.map((t) => sideOf(block, t.side)[t.line]).filter((l) => l !== undefined);
      lines.forEach((text, i) => push(out, text, conflict, i, i === 0));
      if (lines.length === 0) push(out, '(nothing taken)', conflict, null, true, false);
      conflict += 1;
    }
    return out;
  }

  /** Appends a row, numbering it. A placeholder stands in for a line and takes no number. */
  function push(
    out: Row[],
    text: string,
    conflict: number | null,
    line: number | null,
    first: boolean,
    real = true,
  ) {
    const previous = out.at(-1);
    const before = previous?.no ?? 0;
    out.push({ at: out.length, no: real ? before + 1 : null, text, conflict, line, first });
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

  /* The pane holding everything to the right of the file list: a bar, the two sides, the
     result. It scrolls nothing itself; each sheet scrolls on its own. */
  .blocks { flex: 1; min-width: 0; display: flex; flex-direction: column; min-height: 0; }
  .bar {
    flex: 0 0 auto;
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-2) var(--space-3); background: var(--bg-1);
    border-bottom: 1px solid var(--border); font-size: 12px;
  }
  .path { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* Side by side above, the result below: which side to take is a question about the code
     around the conflict, so both sides show the whole file rather than the region alone. */
  .sheets { flex: 3 1 0; display: flex; min-height: 0; border-bottom: 1px solid var(--border-strong); }
  .sheets.frozen { opacity: 0.5; }
  .pane { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .pane + .pane { border-left: 1px solid var(--border); }
  .pane .head {
    flex: 0 0 auto; display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-1) var(--space-2); font-size: 11px;
    background: var(--bg-1); border-bottom: 1px solid var(--border);
  }
  .pane .head .who { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pane .head button { font-size: 10px; flex: 0 0 auto; }
  .tag {
    display: inline-block; width: 15px; text-align: center; border-radius: 3px;
    font-size: 10px; font-weight: 700; color: var(--accent-fg);
  }
  .pane.ours .tag { background: var(--lane-7); }
  .pane.theirs .tag { background: var(--lane-3); }

  .code { flex: 1; min-height: 0; overflow: auto; background: var(--bg-0); }
  .result { flex: 1; min-height: 0; overflow: auto; background: var(--bg-0); }
  .output { flex: 2 1 0; display: flex; flex-direction: column; min-height: 0; }
  .output .bar { border-top: 0; border-bottom: 1px solid var(--border); }

  /* One row per line. `content-visibility` keeps a long file cheap: the rows off screen are
     not laid out at all, which is what makes three sheets of a big file affordable. */
  .line {
    display: flex; align-items: baseline; gap: var(--space-2);
    font-family: var(--font-mono); font-size: 11px; line-height: 17px;
    white-space: pre; content-visibility: auto; contain-intrinsic-size: auto 17px;
  }
  .line .tick { flex: 0 0 14px; text-align: center; }
  .line .tick input { margin: 0; vertical-align: middle; cursor: pointer; }
  /* Wide enough for a four-digit file, with room on the left so the first digit is not
     against the pane's edge. */
  .line .no {
    flex: 0 0 40px; padding-left: var(--space-1); box-sizing: border-box;
    text-align: right; color: var(--fg-2);
    font-variant-numeric: tabular-nums; user-select: none;
  }
  .line .text { flex: 1; min-width: 0; color: var(--fg-0); }

  /* A conflicting line is tinted by which side it is on, and marked when it has been taken.
     The bar down the left is what carries that at a glance while scrolling. */
  .pane.ours .line.in-conflict { background: var(--lane-7-soft); box-shadow: inset 3px 0 0 var(--lane-7); }
  .pane.theirs .line.in-conflict { background: var(--lane-3-soft); box-shadow: inset 3px 0 0 var(--lane-3); }
  .line.in-conflict.taken { font-weight: 600; }
  .result .line.in-conflict { background: var(--lane-2-soft); box-shadow: inset 3px 0 0 var(--lane-2); }
  .result .line.in-conflict.here { outline: 1px solid var(--lane-2); outline-offset: -1px; }

  .typed {
    flex: 1; min-height: 0; width: 100%; box-sizing: border-box; resize: none;
    padding: var(--space-2) var(--space-3); background: var(--bg-0); color: var(--fg-0);
    border: 0; outline: 1px solid var(--accent); outline-offset: -1px;
    font-family: var(--font-mono); font-size: 11px; line-height: 17px; white-space: pre;
  }
  .whole { padding: var(--space-3); font-size: 12px; color: var(--fg-1); max-width: 60ch; }
  .whole p { margin: 0 0 var(--space-3); line-height: 1.5; }
  .choices { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .choices button { font-size: 12px; padding: var(--space-1) var(--space-3); }
  /* A branch name and a commit subject are both long; neither row must grow to fit them. */
  .wholesale button,
  .blocks > .bar > button {
    max-width: 140px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .muted { color: var(--fg-2); padding: var(--space-3); font-size: 12px; }
  .error { color: var(--danger); padding: var(--space-2) var(--space-3); font-size: 12px; margin: 0; }
  .stopped {
    margin: 0; padding: var(--space-2) var(--space-3); font-size: 11px;
    color: var(--warn); background: var(--warn-soft);
    border-bottom: 1px solid var(--border); white-space: pre-wrap;
  }
</style>
