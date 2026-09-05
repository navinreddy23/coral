<script lang="ts">
  import { elidePath } from './path';
  import CodeSheet from './CodeSheet.svelte';
  import { outputRows, sideRows, type Row } from './sheet';
  import type { ConflictedFile } from '../ipc/types';
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

  /** The three files, built once per change rather than once per pane per render. */
  const ourRows = $derived(merge.blocks === null ? [] : sideRows(merge.blocks.blocks, 'ours'));
  const theirRows = $derived(merge.blocks === null ? [] : sideRows(merge.blocks.blocks, 'theirs'));
  const outRows = $derived(
    merge.blocks === null ? [] : outputRows(merge.blocks.blocks, merge.choices),
  );

  /**
   * Width of the number column, in digits of the monospace face.
   *
   * From the longest of the three, so the three sheets line up and the column does not change
   * width as four-digit numbers scroll into it.
   */
  const digits = $derived(
    Math.max(3, String(Math.max(ourRows.length, theirRows.length, outRows.length)).length),
  );

  /**
   * The row each sheet should scroll to, which is how the stepper moves all three at once.
   *
   * A row number rather than an element: the sheets only build what is on screen, so the row
   * the stepper wants usually does not exist yet.
   */
  let jumped = $state(0);
  const ourJump = $derived(rowOfConflict(ourRows, at, jumped));
  const theirJump = $derived(rowOfConflict(theirRows, at, jumped));
  const outJump = $derived(rowOfConflict(outRows, at, jumped));

  function rowOfConflict(rows: Row[], conflict: number, tick: number): number | null {
    void tick;
    const found = rows.findIndex((r) => r.conflict === conflict && r.first);
    return found === -1 ? null : found;
  }

  /**
   * Moves to another conflict and brings it into view in all three sheets.
   *
   * A file with a dozen conflicts is a file where scrolling three sheets by hand is the work,
   * which is why every merge tool has this pair of arrows.
   */
  function step(by: number) {
    const total = merge.conflicts.length;
    if (total === 0) return;
    at = (at + by + total) % total;
    // Changed so the sheets scroll again even when the same conflict is asked for twice.
    jumped += 1;
  }

  /** What to call the operation. The state is an enum name; `cherry_pick` is not a word. */
  const verb = $derived.by(() => {
    const op = merge.operation;
    // `git am` leaves the files a rebase leaves, so the engine reports it as one. Saying
    // "rebase" to somebody who has just opened a patch file names the wrong thing entirely.
    if (op?.applying === true) return 'patch';
    const state = op?.state ?? 'merge';
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
              <CodeSheet
                rows={pane.side === 'ours' ? ourRows : theirRows}
                side={pane.side}
                digits={digits}
                frozen={merge.edited !== null}
                scrollTo={pane.side === 'ours' ? ourJump : theirJump}
                picked={(conflict, line) =>
                  (merge.choices[conflict] ?? []).some(
                    (t) => t.side === pane.side && t.line === line,
                  )}
                onPick={(conflict, line) => merge.toggleLine(conflict, pane.side, line)}
              />
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
            <div class="result">
              {#if outRows.length === 0}
                <p class="muted">The file comes out empty.</p>
              {:else}
                <CodeSheet rows={outRows} here={at} digits={digits} scrollTo={outJump} />
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

  .result { flex: 1; min-height: 0; display: flex; background: var(--bg-0); }
  .output { flex: 2 1 0; display: flex; flex-direction: column; min-height: 0; }
  .output .bar { border-top: 0; border-bottom: 1px solid var(--border); }

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
