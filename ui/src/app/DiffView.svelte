<script lang="ts">
  import { splitRows } from '../diff/split';
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

  const sign: Record<string, string> = { add: '+', remove: '-', context: ' ' };
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

  <div class="scroll">
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
          {#each hunks as hunk (hunk.header + hunk.newStart)}
            <tr class="hunk"><td colspan="4">{hunk.header}</td></tr>
            {#each splitRows(hunk) as row, i (i)}
              <tr>
                <td class="no">{row.left?.oldNo ?? ''}</td>
                <td class="text {row.left ? row.left.kind : 'blank'}">{row.left?.text ?? ''}</td>
                <td class="no">{row.right?.newNo ?? ''}</td>
                <td class="text {row.right ? row.right.kind : 'blank'}">{row.right?.text ?? ''}</td>
              </tr>
            {/each}
          {/each}
        </tbody>
      </table>
    {/if}

    {#if total > LIMIT}
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
  .path {
    flex: 1; min-width: 0; font-size: 12px; color: var(--fg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .tally { flex: 0 0 auto; font-size: 11px; display: flex; gap: var(--space-2); }
  .added { color: var(--ok); font-weight: 600; }
  .removed { color: var(--danger); font-weight: 600; }
  .toggle { display: flex; border: 1px solid var(--border); border-radius: 3px; overflow: hidden; }
  .toggle button {
    font: inherit; font-size: 11px; cursor: pointer; padding: 1px var(--space-2);
    background: var(--bg-0); border: 0; color: var(--fg-1);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on { background: var(--accent); color: var(--accent-fg); font-weight: 600; }
  .close {
    font: inherit; cursor: pointer; background: none; border: 0; color: var(--fg-2);
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
