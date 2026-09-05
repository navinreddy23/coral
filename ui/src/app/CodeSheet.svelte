<script lang="ts">
  /**
   * One file, drawn a line at a time, with only the visible lines in the document.
   *
   * The merge tool lays three of these out. Drawing every line of every one of them put a
   * quarter of a million elements in the page for the kernel's MAINTAINERS file, and the
   * window sat on "Loading…" for minutes. Only what fits on screen is built, in a window
   * translated down a spacer as tall as the file.
   */
  import type { Row } from './sheet';
  import type { Side } from '../state/merge.svelte';

  const {
    rows,
    side = null,
    picked = () => false,
    onPick = () => {},
    here = null,
    frozen = false,
    scrollTo = null,
    digits = 3,
  }: {
    rows: Row[];
    /** Which side this sheet is, or null for the result, which has no boxes to tick. */
    side?: Side | null;
    picked?: (conflict: number, line: number) => boolean;
    onPick?: (conflict: number, line: number) => void;
    /** The conflict the stepper is on, marked out in the result. */
    here?: number | null;
    frozen?: boolean;
    /** A row to bring into view, changed by the stepper. */
    scrollTo?: number | null;
    /** Width of the number column, in digits. */
    digits?: number;
  } = $props();

  /** Line height, in step with the CSS below; the arithmetic here is what places the window. */
  const ROW = 17;
  const OVERSCAN = 12;

  let scroller = $state<HTMLDivElement | null>(null);
  let top = $state(0);
  let viewport = $state(0);

  const first = $derived(Math.max(0, Math.floor(top / ROW) - OVERSCAN));
  const drawn = $derived(rows.slice(first, first + Math.ceil(viewport / ROW) + OVERSCAN * 2));

  // The stepper moves all three sheets at once, and a row it wants may not be built yet, so
  // this scrolls rather than reaching for an element.
  $effect(() => {
    const want = scrollTo;
    if (want === null || scroller === null) return;
    scroller.scrollTop = Math.max(0, want * ROW - viewport / 2);
  });
</script>

<div
  class="code {side ?? 'result'}"
  bind:this={scroller}
  bind:clientHeight={viewport}
  onscroll={(e) => (top = e.currentTarget.scrollTop)}
  style:--gutter="calc({digits}ch + var(--space-3) + var(--space-2))"
>
  <div class="spacer" style:height="{rows.length * ROW}px">
    <div class="window" style:transform="translateY({first * ROW}px)">
      {#each drawn as row (row.at)}
        {@const taken =
          side !== null && row.conflict !== null && row.line !== null && picked(row.conflict, row.line)}
        <div class="line" class:in-conflict={row.conflict !== null} class:taken class:here={row.conflict !== null && row.conflict === here}>
          {#if side !== null}
            <span class="tick">
              {#if row.conflict !== null && row.line !== null}
                {@const conflict = row.conflict}
                {@const line = row.line}
                <input
                  type="checkbox"
                  checked={taken}
                  disabled={frozen}
                  aria-label="Take line {row.no} of this side"
                  onchange={() => onPick(conflict, line)}
                />
              {/if}
            </span>
          {/if}
          <span class="no">{row.no ?? ''}</span>
          <span class="text">{row.text}</span>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .code { flex: 1; min-height: 0; overflow: auto; background: var(--bg-0); }
  /* As tall as the whole file, so the scrollbar tells the truth about its length. */
  .spacer { position: relative; }
  .window { position: absolute; inset: 0 0 auto 0; will-change: transform; }

  .line {
    display: flex; align-items: baseline; gap: var(--space-2); height: 17px;
    font-family: var(--font-mono); font-size: 11px; line-height: 17px; white-space: pre;
  }
  .line .tick { flex: 0 0 14px; text-align: center; }
  .line .tick input { margin: 0; vertical-align: middle; cursor: pointer; }
  /* Sized from the file's own length, with more room on the left than on the right: the
     widest number fills the column exactly, and even padding put its first digit against the
     edge of the pane. */
  .line .no {
    flex: 0 0 var(--gutter); box-sizing: border-box;
    padding: 0 var(--space-2) 0 var(--space-3);
    text-align: right; color: var(--fg-2);
    font-variant-numeric: tabular-nums; user-select: none;
  }
  .line .text { flex: 1; min-width: 0; color: var(--fg-0); overflow: hidden; }

  /* A conflicting line is tinted by which side it is on, and marked when it has been taken.
     The bar down the left is what carries that at a glance while scrolling. */
  .code.ours .line.in-conflict { background: var(--lane-7-soft); box-shadow: inset 3px 0 0 var(--lane-7); }
  .code.theirs .line.in-conflict { background: var(--lane-3-soft); box-shadow: inset 3px 0 0 var(--lane-3); }
  .code.base .line.in-conflict { background: var(--bg-2); }
  .line.in-conflict.taken { font-weight: 600; }
  .code.result .line.in-conflict { background: var(--lane-2-soft); box-shadow: inset 3px 0 0 var(--lane-2); }
  .code.result .line.in-conflict.here { outline: 1px solid var(--lane-2); outline-offset: -1px; }
</style>
