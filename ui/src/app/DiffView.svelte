<script lang="ts">
  import { firstChangedRow, marksOf, splitRows, windowAround } from '../diff/split';
  import { elidePath } from './path';
  import { shortAge } from './age';
  import { initialsOf } from '../graph/initials';
  import type { DiffState } from '../state/diff.svelte';

  const { diff, onClose, onPart }: {
    diff: DiffState;
    onClose: () => void;
    /**
     * Stages, unstages or discards part of a file.
     *
     * `lines` indexes the hunk's own lines; empty means the whole hunk. Raised rather than
     * done here: discarding is the one that cannot be undone, and the window owns the question
     * that has to be answered first.
     */
    onPart: (part: 'stage' | 'unstage' | 'discard', hunk: number, lines: number[]) => void;
  } = $props();

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

  /** Height of one row, matching `--diff-row` in the stylesheet below. */
  const ROW = 17;
  /** Rows kept either side of the viewport, so a fast scroll does not show a gap. */
  const OVERSCAN = 12;

  /**
   * Only the rows on screen are built.
   *
   * A whole file is thousands of rows and four cells each, and a table that size is one
   * composited layer the engine repaints on every wheel notch — which is what made scrolling
   * a side-by-side diff crawl. Rows are a fixed height, so which ones are visible is
   * arithmetic and the rest need not exist.
   */
  let scrolled = $state(0);
  let viewport = $state(400);

  const firstDrawn = $derived(Math.max(0, Math.floor(scrolled / ROW) - OVERSCAN));
  const drawn = $derived(
    split.rows.slice(firstDrawn, firstDrawn + Math.ceil(viewport / ROW) + OVERSCAN * 2),
  );

  /**
   * Width of a line-number gutter, in digits of the monospace face.
   *
   * Fixed rather than sized to its content: with only the visible rows in the document, a
   * column that fits what it holds would change width as four-digit numbers scrolled into it
   * and the whole diff would step sideways. Sized from the last line of the file, not the
   * first: the hunk's own start is line one when the whole file is shown.
   */
  const digits = $derived.by(() => {
    const last = diff.file?.hunks.at(-1);
    if (last === undefined) return 3;
    const highest = Math.max(last.oldStart + last.oldLines, last.newStart + last.newLines);
    return Math.max(3, String(highest).length);
  });

  const sign: Record<string, string> = { add: '+', remove: '-', context: ' ' };

  /**
   * Whether part of this file can be staged, and which way round.
   *
   * Only for the working tree: a commit is written and there is nothing to move in or out of
   * it, and a comparison of two commits is a reading, not a place to work.
   */
  const side = $derived(
    diff.source === 'unstaged' ? 'stage' : diff.source === 'staged' ? 'unstage' : null,
  );

  /**
   * Which lines of which hunk are picked out, keyed `hunk:line`.
   *
   * Cleared whenever the file or the layout changes: a selection is about the lines on screen,
   * and after a reload the same indices are different lines.
   */
  let picked = $state<Set<string>>(new Set());
  let pickedFor = '';
  $effect(() => {
    const key = `${diff.path ?? ''}\u0000${diff.source}\u0000${diff.mode}`;
    void diff.file;
    if (pickedFor !== key) {
      pickedFor = key;
      picked = new Set();
    }
  });

  function togglePick(hunk: number, line: number) {
    const key = `${hunk}:${line}`;
    const next = new Set(picked);
    if (!next.delete(key)) next.add(key);
    picked = next;
  }

  /** The lines picked out of one hunk, in the order git wants them. */
  function pickedIn(hunk: number): number[] {
    return [...picked]
      .filter((key) => key.startsWith(`${hunk}:`))
      .map((key) => Number(key.slice(key.indexOf(':') + 1)))
      .sort((a, b) => a - b);
  }

  /** A line worth picking: context is in both files and cannot be staged on its own. */
  function pickable(kind: string): boolean {
    return kind !== 'context';
  }

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
    const key = `${diff.path ?? ''}\u0000${diff.mode}\u0000${diff.atCommit ?? ''}`;
    const rows = split.rows;
    const el = scroller;
    if (diff.mode !== 'split' || el === null || rows.length === 0 || placed === key) return;
    placed = key;
    const at = firstChangedRow(rows);
    if (at < 0) return;
    el.scrollTop = Math.max(0, at * ROW - el.clientHeight / 3);
  });

  function onScroll(event: Event) {
    scrolled = (event.currentTarget as HTMLElement).scrollTop;
  }

  /**
   * Where the changes are in the file, for the strip down the side.
   *
   * A whole-file diff is mostly unchanged text and the scrollbar says nothing about where the
   * few changed lines are; this is the map of them, and clicking it goes there.
   */
  const marks = $derived(marksOf(split.rows));

  /** The part of the file on screen, drawn over the marks so the strip says where you are. */
  const here = $derived.by(() => {
    const height = split.rows.length * ROW;
    if (height <= 0) return { at: 0, size: 1 };
    return { at: Math.min(1, scrolled / height), size: Math.min(1, viewport / height) };
  });

  /** Where each changed run starts, as a row, for the next and previous buttons. */
  const changeRows = $derived(marks.map((mark) => Math.round(mark.at * split.rows.length)));

  /**
   * Moves to the change before or after the one on screen.
   *
   * Measured a third of the way down the viewport rather than from its top edge, so pressing
   * "next" while a change is half off the bottom goes to that one rather than past it.
   */
  function step(direction: 1 | -1) {
    const el = scroller;
    if (el === null || changeRows.length === 0) return;
    const at = (el.scrollTop + el.clientHeight / 3) / ROW;
    const next =
      direction === 1
        ? changeRows.find((row) => row > at + 1)
        : [...changeRows].reverse().find((row) => row < at - 1);
    if (next === undefined) return;
    el.scrollTop = Math.max(0, next * ROW - el.clientHeight / 3);
  }

  /** The file's lines paired with the commit that last changed each. */
  const blamed = $derived.by(() => {
    const blame = diff.blame;
    const text = diff.text;
    if (blame === null || text === null) return [];

    const by = new Map<number, (typeof blame.chunks)[number]>();
    for (const chunk of blame.chunks) by.set(chunk.finalLine, chunk);

    const lines = text.split('\n');
    if (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
    return lines.map((content, i) => {
      const chunk = by.get(i + 1);
      const commit = chunk === undefined ? undefined : blame.commits[chunk.oid];
      return {
        no: i + 1,
        content,
        // Only the first line of a run carries the chip; a column repeating one name down
        // twenty lines is what makes a blame unreadable.
        oid: chunk?.oid ?? null,
        who: commit?.author.name ?? '',
        when: commit?.author.time ?? 0,
        summary: commit?.summary ?? '',
      };
    });
  });

  function jump(event: MouseEvent) {
    const el = scroller;
    if (el === null) return;
    const strip = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (event.clientY - strip.top) / strip.height));
    const height = split.rows.length * ROW;
    el.scrollTop = Math.max(0, fraction * height - el.clientHeight / 2);
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
    <!-- Not for a comparison of two commits: blame and history are about one file's past,
         and there is no single revision here to have one. -->
    {#if diff.source !== 'compare'}
      <div class="toggle" role="group" aria-label="What to show about this file">
        <button class:on={diff.view === 'diff'} onclick={() => diff.setView('diff')}>Diff</button>
        <button class:on={diff.view === 'blame'} onclick={() => diff.setView('blame')}>Blame</button>
        <button class:on={diff.view === 'history'} onclick={() => diff.setView('history')}>
          History
        </button>
      </div>
    {/if}

    {#if diff.view === 'diff'}
      <div class="toggle" role="group" aria-label="Diff layout">
        <button class:on={diff.mode === 'inline'} onclick={() => diff.setMode('inline')}>
          Inline
        </button>
        <button class:on={diff.mode === 'split'} onclick={() => diff.setMode('split')}>
          Side by side
        </button>
      </div>

      {#if diff.mode === 'split'}
        <div class="steps">
          <button
            onclick={() => step(-1)}
            disabled={changeRows.length === 0}
            title="Previous change">↑</button
          >
          <button
            onclick={() => step(1)}
            disabled={changeRows.length === 0}
            title="Next change">↓</button
          >
        </div>
      {/if}

      <!-- A reformatting commit rewrites a file without changing what it says. -->
      <button
        class="ws"
        class:on={diff.ignoreWhitespace}
        onclick={() => diff.setIgnoreWhitespace(!diff.ignoreWhitespace)}
        title={diff.ignoreWhitespace ? 'Counting whitespace again' : 'Ignore whitespace'}
        aria-pressed={diff.ignoreWhitespace}
      >¶</button>
    {/if}
    <button class="close" onclick={onClose} aria-label="Close the diff">✕</button>
  </header>

  <div class="body">
  {#if diff.view === 'history'}
    <!--
      The commits that touched this file, newest first. Selecting one shows what it did to
      the file, which is the question a file history is opened to answer.
    -->
    <ul class="history">
      {#each diff.history as commit (commit.oid)}
        <li>
          <button
            class="entry"
            class:on={diff.atCommit === commit.oid}
            onclick={() => void diff.showCommit(commit.oid)}
          >
            <span class="who" title={commit.author.name}>{initialsOf(commit.author.name) ?? '?'}</span>
            <span class="what">
              <span class="subject">{commit.summary}</span>
              <span class="by">{shortAge(commit.author.time)} ago by {commit.author.name}</span>
            </span>
            <span class="sha mono">{commit.oid.slice(0, 7)}</span>
          </button>
        </li>
      {/each}
      {#if diff.history.length === 0}
        <li class="muted">Nothing has touched this file.</li>
      {:else if diff.moreHistory}
        <li>
          <button class="deeper" onclick={() => void diff.deeper()}>Go deeper</button>
        </li>
      {/if}
    </ul>
  {/if}

  <div class="scroll" bind:this={scroller} onscroll={onScroll} bind:clientHeight={viewport}>
    {#if diff.view === 'blame'}
      {#if diff.blame === null || diff.text === null}
        <p class="muted">Working out who wrote each line…</p>
      {:else}
        <table class="lines blame">
          <tbody>
            {#each blamed as line (line.no)}
              <tr>
                <td class="author">
                  {#if line.oid !== null}
                    <span class="chip" title="{line.summary}&#10;{line.who}">
                      {line.who}
                      <span class="mono">{line.oid.slice(0, 7)}</span>
                      <span class="when">{shortAge(line.when)}</span>
                    </span>
                  {/if}
                </td>
                <td class="no">{line.no}</td>
                <td class="text">{line.content}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else if diff.loading}
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
          {#each hunks as hunk, h (hunk.header + hunk.newStart)}
            <tr class="hunk">
              <td colspan="3">
                <div class="hunk-bar">
                <span class="where">{hunk.header}</span>
                {#if side !== null}
                  <span class="hunk-acts">
                    {#if pickedIn(h).length > 0}
                      <button onclick={() => onPart(side, h, pickedIn(h))}>
                        {side === 'stage' ? 'Stage' : 'Unstage'} {pickedIn(h).length} line{pickedIn(h).length === 1 ? '' : 's'}
                      </button>
                    {/if}
                    <button onclick={() => onPart(side, h, [])}>
                      {side === 'stage' ? 'Stage hunk' : 'Unstage hunk'}
                    </button>
                    {#if side === 'stage'}
                      <button class="risky" onclick={() => onPart('discard', h, pickedIn(h))}>
                        Discard{pickedIn(h).length > 0 ? ' lines' : ' hunk'}
                      </button>
                    {/if}
                  </span>
                {/if}
                </div>
              </td>
            </tr>
            {#each hunk.lines as line, i (i)}
              <tr class={line.kind} class:picked={picked.has(`${h}:${i}`)}>
                <td class="no">{line.oldNo ?? ''}</td>
                <td class="no">{line.newNo ?? ''}</td>
                {#if side !== null && pickable(line.kind)}
                  <td class="text">
                    <button class="pick" onclick={() => togglePick(h, i)} title="Pick this line"
                      ><span class="sign">{sign[line.kind]}</span>{line.text}</button
                    >
                  </td>
                {:else}
                  <td class="text"><span class="sign">{sign[line.kind]}</span>{line.text}</td>
                {/if}
              </tr>
            {/each}
          {/each}
        </tbody>
      </table>
    {:else}
      <div
        class="sheet"
        style:height="{split.rows.length * ROW}px"
        style:--gutter="calc({digits}ch + var(--space-3) + var(--space-2))"
      >
        <div class="window" style:transform="translateY({firstDrawn * ROW}px)">
          {#each drawn as row, i (firstDrawn + i)}
            <div class="line">
              <span class="no">{row.left?.oldNo ?? ''}</span>
              <span class="cell {row.left ? row.left.kind : 'blank'}">{row.left?.text ?? ''}</span>
              <span class="no">{row.right?.newNo ?? ''}</span>
              <span class="cell {row.right ? row.right.kind : 'blank'}"
                >{row.right?.text ?? ''}</span
              >
            </div>
          {/each}
        </div>
      </div>
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

  {#if diff.mode === 'split' && marks.length > 0}
    <button
      class="overview"
      onclick={jump}
      aria-label="Go to a change in the file"
      title="{marks.length} change{marks.length === 1 ? '' : 's'} in this file"
    >
      {#each marks as mark, i (i)}
        <span
          class="mark {mark.kind}"
          style:top="{mark.at * 100}%"
          style:height="max(2px, {mark.size * 100}%)"
        ></span>
      {/each}
      <span class="here" style:top="{here.at * 100}%" style:height="{here.size * 100}%"></span>
    </button>
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

  .body { flex: 1; min-height: 0; display: flex; }

  .steps { display: flex; gap: 2px; }
  .steps button, .ws {
    font: inherit; font-size: 12px; cursor: pointer; line-height: 18px;
    background: var(--bg-0); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1); padding: 0 6px;
  }
  .steps button:hover:not(:disabled), .ws:hover { background: var(--bg-2); color: var(--fg-0); }
  .steps button:disabled { color: var(--fg-2); cursor: default; }
  .ws.on { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }

  /*
   * The file's history, beside the change it made rather than above it: the list is scrolled
   * down while one entry's diff is read, and a list that moved out from under the diff would
   * cost the reader their place every time they stepped back one commit.
   */
  .history {
    flex: 0 0 auto; width: 300px; overflow-y: auto; list-style: none; margin: 0;
    padding: var(--space-1); border-right: 1px solid var(--border); background: var(--bg-1);
  }
  .entry {
    display: flex; align-items: center; gap: var(--space-2); width: 100%; text-align: left;
    font: inherit; font-size: 12px; cursor: pointer; color: var(--fg-1);
    background: var(--bg-1); border: 0; border-radius: var(--radius-1);
    padding: var(--space-1) var(--space-2); overflow: hidden;
  }
  .entry:hover { background: var(--bg-2); }
  .entry.on { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line); }
  .who {
    flex: 0 0 auto; width: 24px; height: 24px; border-radius: var(--radius-1);
    display: flex; align-items: center; justify-content: center;
    background: var(--node-1); color: #ffffff; font-size: 10px; font-weight: 600;
  }
  .what { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .subject { color: var(--fg-0); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .by { color: var(--fg-2); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sha { flex: 0 0 auto; color: var(--fg-2); font-size: 11px; }
  .deeper {
    width: 100%; font: inherit; font-size: 11px; cursor: pointer; margin-top: var(--space-2);
    background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    color: var(--fg-1); padding: var(--space-1);
  }
  .deeper:hover { background: var(--bg-3); color: var(--fg-0); }

  /* Who wrote each line, named once per run rather than once per line. */
  .blame .author {
    width: 1%; white-space: nowrap; user-select: none; vertical-align: top;
    padding: 0 var(--space-2); background: var(--bg-1);
    border-right: 1px solid var(--border);
  }
  .chip {
    display: inline-flex; gap: var(--space-2); align-items: baseline;
    font-family: var(--font-ui); font-size: 11px; color: var(--fg-1);
  }
  .chip .when { color: var(--fg-2); }
  .scroll { flex: 1; min-width: 0; overflow: auto; }

  /*
   * The whole file in one column: where every change is, and where the reader is among them.
   * A scrollbar on a diff of two thousand lines says how far down the file it is and nothing
   * about the four lines that changed.
   */
  .overview {
    flex: 0 0 auto; width: 14px; position: relative; cursor: pointer;
    border: 0; border-left: 1px solid var(--border); padding: 0;
    background: var(--bg-1);
  }
  .overview:hover { background: var(--bg-2); }
  .mark { position: absolute; left: 2px; right: 2px; border-radius: 1px; }
  .mark.add { background: var(--ok); }
  .mark.remove { background: var(--danger); }
  .mark.both { background: var(--lane-3); }
  /* An outline rather than a fill: the marks under it are the point of the strip. */
  .here {
    position: absolute; left: 0; right: 0; min-height: 8px;
    border: 1px solid var(--fg-2); border-radius: 2px; background: none;
  }
  .lines {
    border-collapse: collapse; width: 100%;
    font-family: var(--font-mono); font-size: 11px; font-weight: 400; line-height: 17px;
  }
  /*
   * A gutter has to look like one. At `--bg-1` it is three percent off the page the code sits
   * on, which at a glance is no strip at all: the numbers read as floating in the text rather
   * than as sitting beside it, and a long number looks like it has escaped into the margin.
   *
   * `td`, and that is the whole of the line-number bug. `width: 1%` is how a table column is
   * asked to shrink to its content, and the side-by-side sheet below names its cells `no` too.
   * Unscoped, this rule reached them as well: each sat one percent wide inside a grid track
   * that had been sized correctly all along, so the strip stopped at the padding and every
   * number past two digits was drawn half on it and half on the code.
   */
  td.no {
    width: 1%; white-space: nowrap; text-align: right; user-select: none;
    padding: 0 var(--space-2) 0 var(--space-3); color: var(--fg-2); background: var(--bg-2);
    border-right: 1px solid var(--border-strong);
    /* Numbers only ever read down the column, so they line up. */
    font-variant-numeric: tabular-nums;
  }
  /* Long lines scroll with the table rather than wrapping: a wrapped diff loses the one-line,
     one-row correspondence that makes the two columns comparable. */
  /*
   * An opaque background of its own, and this is not optional. WebKit antialiases text on a
   * composited layer with subpixel precision only where it knows what is behind it; left
   * transparent it falls back to grayscale, which at eleven pixels of a monospace face reads
   * as bold rather than as soft. The add, remove and blank tints below override it.
   */
  .text { white-space: pre; padding: 0 var(--space-2); color: var(--fg-0); background: var(--bg-0); }

  /*
   * The side-by-side sheet: a spacer as tall as the whole file, with only the visible rows
   * inside it. See the comment on `drawn` for why it is not a table.
   */
  .sheet {
    position: relative; overflow: hidden;
    font-family: var(--font-mono); font-size: 11px; font-weight: 400; line-height: 17px;
  }
  .window { position: absolute; inset: 0 0 auto 0; will-change: transform; }
  /*
   * `minmax(0, 1fr)`, not `1fr`. A bare `1fr` is `minmax(auto, 1fr)`, so a column whose
   * content cannot wrap — a long line of code under `white-space: pre` — grows past its share
   * and pushes everything after it along. That is what put the right pane's gutter in the
   * middle of the left pane's text, and its code off the side of the window: the numbers
   * looked like they had escaped into the margin because the column holding them had.
   */
  .line {
    display: grid; height: 17px;
    grid-template-columns: var(--gutter) minmax(0, 1fr) var(--gutter) minmax(0, 1fr);
  }
  /* More room on the left than on the right: the widest number the file reaches fills the
     column exactly, and with even padding its first digit landed against the pane's edge. */
  .line .no {
    text-align: right; user-select: none; padding: 0 var(--space-2) 0 var(--space-3);
    color: var(--fg-2); background: var(--bg-2);
    border-right: 1px solid var(--border-strong); box-sizing: border-box;
    font-variant-numeric: tabular-nums;
  }
  .cell {
    white-space: pre; overflow: hidden; padding: 0 var(--space-2);
    color: var(--fg-0); background: var(--bg-0);
  }
  .cell.add { background: var(--add-bg); box-shadow: inset 2px 0 0 var(--ok); }
  .cell.remove { background: var(--remove-bg); box-shadow: inset 2px 0 0 var(--danger); }
  .cell.blank { background: var(--bg-1); }
  .sign { user-select: none; color: var(--fg-2); }
  tr.add .text { background: var(--add-bg); box-shadow: inset 2px 0 0 var(--ok); }
  tr.remove .text { background: var(--remove-bg); box-shadow: inset 2px 0 0 var(--danger); }
  /*
   * The hunk header is a divider with a location on it, not a line of the file. Ruled above
   * and below so a long diff reads as a sequence of regions rather than one wall.
   */
  /* The hunk header carries its own actions, so it is a bar rather than a caption. The flex
     row is inside the cell, not the cell itself: `display: flex` on a `td` takes it out of
     table layout and `colspan` stops meaning anything, which left the bar a third as wide as
     the diff under it. */
  .hunk-bar { display: flex; align-items: center; gap: var(--space-3); }
  .where { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .hunk-acts { flex: 0 0 auto; display: flex; gap: var(--space-1); }
  .hunk-acts button {
    font: inherit; font-family: var(--font-ui); font-size: 11px; cursor: pointer;
    background: var(--bg-0); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1); padding: 0 6px; line-height: 17px;
  }
  .hunk-acts button:hover { background: var(--bg-3); color: var(--fg-0); }
  .hunk-acts button.risky { color: var(--danger); }
  .hunk-acts button.risky:hover { background: var(--danger-soft); }

  /* A changed line is a target, because picking lines is how part of a hunk gets staged. */
  .pick {
    display: block; width: 100%; text-align: left; font: inherit; cursor: pointer;
    background: none; border: 0; padding: 0; color: inherit; white-space: pre;
  }
  tr.picked .text { box-shadow: inset 3px 0 0 var(--accent); }
  tr.picked .sign { color: var(--accent); font-weight: 700; }

  tr.hunk td {
    background: var(--bg-2); color: var(--fg-2); padding: 3px var(--space-2);
    white-space: pre; user-select: none;
    border-top: 1px solid var(--border); border-bottom: 1px solid var(--border);
    font-size: 10px; letter-spacing: 0.02em;
  }
  .muted { color: var(--fg-2); padding: var(--space-3); font-size: 12px; }
  .error { color: var(--danger); padding: var(--space-3); font-size: 12px; }
</style>
