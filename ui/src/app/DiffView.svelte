<script lang="ts">
  import { splitRows, windowAround } from '../diff/split';
  import { elidePath } from './path';
  import type { DiffState } from '../state/diff.svelte';

  const { diff, onClose }: { diff: DiffState; onClose: () => void } = $props();

  /**
   * Lines rendered before the rest is summarised.
   *
   * A generated file can run to hundreds of thousands of lines in one hunk, and every line is
   * a DOM row here; past this it is not a diff anyone is reading, it is a scroll bar.
   */
  const LIMIT = 6000;

  const total = $derived(
    (diff.file?.hunks ?? []).reduce((n, h) => n + h.lines.length, 0),
  );

  /** Hunks trimmed to the line budget, so a huge file still shows its first change. */
  const hunks = $derived.by(() => {
    const all = diff.file?.hunks ?? [];
    if (total <= LIMIT) return all;
    const out = [];
    let budget = LIMIT;
    for (const hunk of all) {
      if (budget <= 0) break;
      out.push(hunk.lines.length <= budget ? hunk : { ...hunk, lines: hunk.lines.slice(0, budget) });
      budget -= hunk.lines.length;
    }
    return out;
  });

  /**
   * The whole file as side-by-side rows, windowed to the budget.
   *
   * Side by side asks git for the file end to end rather than for hunks, so there is normally
   * one hunk here holding all of it and no `@@` header worth showing. Past the budget the
   * window is taken around the first change rather than from the top, or a long file would
   * open on a screen of unchanged lines.
   */
  const split = $derived.by(() => {
    const all = (diff.file?.hunks ?? []).flatMap(splitRows);
    const { rows, from } = windowAround(all, LIMIT);
    return { rows, from, total: all.length };
  });

  const sign: Record<string, string> = { add: '+', remove: '-', context: ' ' };

  let scroller: HTMLElement | null = $state(null);
  /** Which file and layout the view has already been positioned for. */
  let placed = '';

  /**
   * Puts the first change on screen when the whole file is shown.
   *
   * Side by side starts at line one, and the first change in a kernel defconfig is on line
   * 1428: left where it opens, the panel shows a screen of unchanged text and the reader has
   * to go looking for what the commit did. Only when the file or the layout changes, so an
   * editor's autosave reloading the diff does not drag the view back while it is being read.
   */
  $effect(() => {
    const key = `${diff.path ?? ''}\u0000${diff.mode}`;
    const file = diff.file;
    const el = scroller;
    if (!file || diff.mode !== 'split' || el === null || placed === key) return;
    placed = key;
    requestAnimationFrame(() => reveal(el));
  });

  function reveal(el: HTMLElement) {
    const cell = el.querySelector('td.add, td.remove');
    const row = cell?.closest('tr');
    if (!row) return;
    // Measured rather than computed from a row height: the table's rows are uniform now and
    // an assumption that they stay so is one nobody would think to check.
    const top = row.getBoundingClientRect().top - el.getBoundingClientRect().top + el.scrollTop;
    el.scrollTop = Math.max(0, top - el.clientHeight / 3);
  }
</script>

<section class="diff">
  <header>
    <span class="path mono" title={diff.path ?? ''}>{elidePath(diff.path ?? '', 72)}</span>
    {#if diff.file && !diff.file.binary}
      <span class="tally">
        <span class="added">+{diff.file.added ?? 0}</span>
        <span class="removed">−{diff.file.removed ?? 0}</span>
      </span>
    {/if}
    <div class="toggle" role="group" aria-label="Diff layout">
      <button class:on={diff.mode === 'inline'} onclick={() => diff.setMode('inline')}>
        Inline
      </button>
      <button class:on={diff.mode === 'split'} onclick={() => diff.setMode('split')}>
        Side by side
      </button>
    </div>
    <button class="close" onclick={onClose} aria-label="Close the diff">✕</button>
  </header>

  <div class="scroll" bind:this={scroller}>
    {#if diff.loading}
      <p class="muted">Loading…</p>
    {:else if diff.error}
      <p class="error">{diff.error}</p>
    {:else if !diff.file}
      <p class="muted">Nothing to show.</p>
    {:else if diff.file.binary}
      <p class="muted">Binary file — no textual diff.</p>
    {:else if diff.file.tooLarge}
      <p class="muted">The file is past the size guard, so its contents were not read.</p>
    {:else if diff.file.hunks.length === 0}
      <p class="muted">
        {diff.file.oldPath ? `Renamed from ${diff.file.oldPath}.` : 'No line changes.'}
      </p>
    {:else if diff.mode === 'inline'}
      <table class="lines">
        <tbody>
          {#each hunks as hunk (hunk.header + hunk.newStart)}
            <tr class="hunk"><td colspan="3">{hunk.header}</td></tr>
            {#each hunk.lines as line, i (i)}
              <tr class={line.kind}>
                <td class="no">{line.oldNo ?? ''}</td>
                <td class="no">{line.newNo ?? ''}</td>
                <td class="text"><span class="sign">{sign[line.kind]}</span>{line.text}</td>
              </tr>
            {/each}
          {/each}
        </tbody>
      </table>
    {:else}
      <table class="lines split">
        <tbody>
          {#each split.rows as row, i (i)}
            <tr>
              <td class="no">{row.left?.oldNo ?? ''}</td>
              <td class="text {row.left ? row.left.kind : 'blank'}">{row.left?.text ?? ''}</td>
              <td class="no">{row.right?.newNo ?? ''}</td>
              <td class="text {row.right ? row.right.kind : 'blank'}">{row.right?.text ?? ''}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    {#if diff.mode === 'split' && split.total > split.rows.length}
      <p class="muted">
        Showing lines {(split.from + 1).toLocaleString()} to
        {(split.from + split.rows.length).toLocaleString()} of {split.total.toLocaleString()},
        around the first change.
      </p>
    {:else if diff.mode === 'inline' && total > LIMIT}
      <p class="muted">
        Showing the first {LIMIT.toLocaleString()} of {total.toLocaleString()} lines.
      </p>
    {/if}
  </div>
</section>

<style>
  .diff { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--bg-0); }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    height: 34px; padding: 0 var(--space-3); flex: 0 0 auto;
    border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  /*
   * Every text surface in the header paints its own background, as everywhere else in the
   * window: WebKit antialiases with subpixel precision on a composited layer only where it
   * knows what is behind, and heavy text is where the fallback to grayscale shows first.
   */
  .path, .tally, .added, .removed { background: var(--bg-1); }
  /* Normal weight. The file name is a label on the diff, not a heading over it, and mono at
     twelve pixels already reads heavier than the interface font beside it. */
  .path {
    flex: 1; min-width: 0; font-size: 12px; font-weight: 400; color: var(--fg-1);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .tally { flex: 0 0 auto; font-size: 11px; display: flex; gap: var(--space-2); }
  .added { color: var(--ok); }
  .removed { color: var(--danger); }
  .toggle { display: flex; border: 1px solid var(--border); border-radius: 3px; overflow: hidden; }
  .toggle button {
    font: inherit; font-size: 11px; cursor: pointer; padding: 1px var(--space-2);
    background: var(--bg-0); border: 0; color: var(--fg-1);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on { background: var(--accent); color: var(--accent-fg); font-weight: 600; }
  .close {
    font: inherit; cursor: pointer; background: var(--bg-0); border: 0; color: var(--fg-2);
    padding: 2px var(--space-2); border-radius: var(--radius-1);
  }
  .close:hover { color: var(--fg-0); background: var(--bg-2); }

  .scroll { flex: 1; overflow: auto; }
  .lines {
    border-collapse: collapse; width: 100%;
    font-family: var(--font-mono); font-size: 11px; line-height: 17px;
  }
  .no {
    width: 1%; white-space: nowrap; text-align: right; user-select: none;
    padding: 0 var(--space-2); color: var(--fg-2); background: var(--bg-1);
    border-right: 1px solid var(--border);
    /* Numbers only ever read down the column, so they line up. */
    font-variant-numeric: tabular-nums;
  }
  /* Long lines scroll with the table rather than wrapping: a wrapped diff loses the one-line,
     one-row correspondence that makes the two columns comparable. */
  .text { white-space: pre; padding: 0 var(--space-2); color: var(--fg-0); }
  .sign { user-select: none; color: var(--fg-2); }
  tr.add .text, td.text.add {
    background: var(--add-bg); box-shadow: inset 2px 0 0 var(--ok);
  }
  tr.remove .text, td.text.remove {
    background: var(--remove-bg); box-shadow: inset 2px 0 0 var(--danger);
  }
  td.text.blank { background: var(--bg-1); }
  /*
   * The hunk header is a divider with a location on it, not a line of the file. Ruled above
   * and below so a long diff reads as a sequence of regions rather than one wall.
   */
  tr.hunk td {
    background: var(--bg-2); color: var(--fg-2); padding: 3px var(--space-2);
    white-space: pre; user-select: none;
    border-top: 1px solid var(--border); border-bottom: 1px solid var(--border);
    font-size: 10px; letter-spacing: 0.02em;
  }
  .split td.text { width: 50%; max-width: 0; overflow: hidden; }
  .muted { color: var(--fg-2); padding: var(--space-3); font-size: 12px; }
  .error { color: var(--danger); padding: var(--space-3); font-size: 12px; }
</style>
