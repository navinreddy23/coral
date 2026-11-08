<script lang="ts">
  import ImageDiff, { isImage } from './ImageDiff.svelte';
  import Icon from './Icon.svelte';
  import { firstChangedRow, marksOf, onScreen, splitRows, windowAround } from '../diff/split';
  import { gapsOf, type Gap } from '../diff/gap';
  import { wordMarks } from '../diff/word';
  import CodeLine from './CodeLine.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import { copyText } from './clipboard';
  import { elidePath } from './path';
  import { shortAge } from './age';
  import { initialsOf } from '../graph/initials';
  import type { DiffState } from '../state/diff.svelte';
  import type { SyntaxState } from '../state/syntax.svelte';
  import { grammarOf } from '../syntax/grammar';
  import type { Token } from '../syntax/token';
  import type { FileDiff, Line } from '../ipc/types';

  const { diff, syntax = null, onClose, onPart, onEdit = () => undefined, onShowCommit = null }: {
    diff: DiffState;
    /** Code colours, or null to draw every line plain. */
    syntax?: SyntaxState | null;
    onClose: () => void;
    /**
     * Stages, unstages or discards part of a file.
     *
     * `lines` indexes the hunk's own lines; empty means the whole hunk. Raised rather than
     * done here: discarding is the one that cannot be undone, and the window owns the question
     * that has to be answered first.
     */
    onPart: (part: 'stage' | 'unstage' | 'discard', hunk: number, lines: number[]) => void;
    /** Stage a hunk as rewritten: its new side, context and added lines, as text to edit. */
    onEdit?: (hunk: number, text: string) => void;
    /** Goes to a commit a blame names, in the graph; null where there is no graph to go to. */
    onShowCommit?: ((oid: string) => void) | null;
  } = $props();

  /**
   * Lines rendered before the rest is summarised.
   *
   * A generated file can run to hundreds of thousands of lines in one hunk, and every line is
   * a DOM row here; past this it is not a diff anyone is reading, it is a scroll bar.
   */
  const LIMIT = 6000;

  /**
   * What to say about a file the panel has no lines for.
   *
   * Three different things end here — a rename that moved nothing, a file added with nothing
   * in it, a change that touched only the mode — and "No line changes." was all three.
   */
  function emptyDiffSays(file: FileDiff): string {
    if (file.oldPath) return `Renamed from ${file.oldPath}.`;
    if (file.change === 'added') return 'A new file with nothing in it.';
    return 'No line changes.';
  }

  /**
   * What git says about a file whose last line has no newline after it.
   *
   * The diff carries the flag and the panel dropped it, so a commit that only added a trailing
   * newline drew "gamma" removed and "gamma" added with nothing on screen saying what differs.
   */
  const NO_NEWLINE = 'No newline at end of file';

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

  /**
   * What changed inside each replaced line, keyed by the line itself.
   *
   * Computed once per file rather than per rendered row: both layouts look their lines up in
   * the same map, and side by side re-renders its rows on every scroll frame.
   */
  const words = $derived(wordMarks(diff.file?.hunks ?? []));

  /**
   * Colours, asked for as each view needs them and drawn once they are back.
   *
   * The diff is coloured from the two whole files, a line at a time by its number on its own
   * side; a line whose text is not that side's line — a context line git printed from the other
   * side under ignored whitespace — is left plain rather than coloured from the wrong text.
   */
  const grammar = $derived(diff.path === null ? null : grammarOf(diff.path));
  // Both whole sides, for the colours and for the lines between hunks, whichever is wanted.
  $effect(() => {
    if (diff.view === 'diff' || diff.view === 'history') void diff.readSides();
  });
  $effect(() => {
    if (syntax === null || !syntax.on || grammar === null) return;
    if (diff.view === 'blame') syntax.want(grammar, diff.text);
    else if (diff.view === 'file') syntax.want(grammar, diff.contents?.kind === 'text' ? diff.contents.text : null);
    else {
      syntax.want(grammar, diff.sideTexts?.before ?? null);
      syntax.want(grammar, diff.sideTexts?.after ?? null);
    }
  });
  const before = $derived(sideOf(diff.sideTexts?.before ?? null));
  const after = $derived(sideOf(diff.sideTexts?.after ?? null));

  function sideOf(text: string | null): { lines: string[]; tokens: Token[][] } | null {
    const tokens = syntax?.tokens(grammar, text) ?? null;
    return text === null || tokens === null ? null : { lines: text.split('\n'), tokens };
  }

  /** A diff line's colours, from the side it is on, or null to draw it plain. */
  function coloursOf(line: Line | null, side: 'before' | 'after'): Token[] | null {
    const from = side === 'before' ? before : after;
    const no = side === 'before' ? line?.oldNo : line?.newNo;
    if (line === null || from === null || no === null || no === undefined) return null;
    return from.lines[no - 1] === line.text ? (from.tokens[no - 1] ?? null) : null;
  }

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
  // None over a commit picked from the file's history, though the panel was opened on the
  // working copy: a hunk's index is read against the working copy, so acting on what is shown
  // would stage or throw away a different hunk from the one on screen.
  // None while whitespace is ignored either: the hunks on screen are not the file's, and a
  // patch read past whitespace does not apply to it.
  const side = $derived(
    diff.atCommit !== null || diff.ignoreWhitespace
      ? null
      : diff.source === 'unstaged'
        ? 'stage'
        : diff.source === 'staged'
          ? 'unstage'
          : null,
  );

  /**
   * Which lines of which hunk are picked out, keyed `hunk:line`.
   *
   * Cleared whenever the file or the layout changes: a selection is about the lines on screen,
   * and after a reload the same indices are different lines.
   */
  let picked = $state<Set<string>>(new Set());
  let pickedFor = '';
  let pickedOn: unknown = null;
  $effect(() => {
    const key = `${diff.path ?? ''}\u0000${diff.source}\u0000${diff.mode}`;
    // The diff object itself, not only the key: staging part of a hunk reloads the same file
    // in the same mode, and the hunks that come back are different lines under the same
    // indices. Kept, the bar went on offering to stage a line nobody had picked, and git
    // answered the patch built from it with "corrupt patch at line 12".
    const file = diff.file;
    if (pickedFor !== key || pickedOn !== file) {
      pickedFor = key;
      pickedOn = file;
      picked = new Set();
      anchor = null;
      opened = {};
    }
  });

  /** Lines a gap opens by, from either end. */
  const STEP = 20;

  /**
   * How much of each gap between hunks has been opened, from its top and from its bottom,
   * keyed by the hunk it sits above.
   *
   * Drawn as rows of their own between the hunks, never put into a hunk's lines: staging part
   * of a hunk names its lines by their place in it, and a line opened above them would move
   * every one.
   */
  let opened = $state<Record<number, { top: number; bottom: number }>>({});

  /**
   * The gaps the inline view can open, by the hunk each sits above.
   *
   * Measured against the new side as it stood for this answer from git, and only once that has
   * been read: the old one after a reload would open onto the wrong lines. None when the whole
   * file is already on screen, or the hunks were cut short at the line budget.
   */
  const gaps = $derived.by(() => {
    const sides = diff.sideTexts;
    const none = new Map<number, Gap>();
    if (diff.mode !== 'inline' || diff.expanded || total > LIMIT) return none;
    if (sides === null || sides.file !== diff.file || sides.after === null) return none;
    const lines = linesOf(sides.after);
    return new Map(gapsOf(hunks, lines.length).map((gap) => [gap.below, gap]));
  });
  const trailingGap = $derived(gaps.get(hunks.length));
  const afterText = $derived(
    diff.sideTexts !== null && diff.sideTexts.after !== null ? linesOf(diff.sideTexts.after) : [],
  );

  function openGap(gap: Gap, how: 'top' | 'bottom' | 'all') {
    const now = opened[gap.below] ?? { top: 0, bottom: 0 };
    const size = gap.last - gap.first + 1;
    const next =
      how === 'all'
        ? { top: size, bottom: 0 }
        : how === 'top'
          ? { ...now, top: now.top + STEP }
          : { ...now, bottom: now.bottom + STEP };
    opened = { ...opened, [gap.below]: next };
  }

  /** The new-side numbers from `from` to `to`, both included; none when `to` is before `from`. */
  function numbers(from: number, to: number): number[] {
    return Array.from({ length: Math.max(0, to - from + 1) }, (_, i) => from + i);
  }

  /** An opened line of a gap, as the line it is on both sides. */
  function gapLine(no: number, shift: number): Line {
    return { kind: 'context', text: afterText[no - 1] ?? '', oldNo: no + shift, newNo: no, noNewline: false };
  }

  /** The line last clicked, which a Shift-click picks from. */
  let anchor: { hunk: number; line: number } | null = null;

  function togglePick(event: MouseEvent, hunk: number, line: number) {
    const next = new Set(picked);
    const from = anchor;
    if (event.shiftKey && from !== null && from.hunk === hunk) {
      // A run is picked as the graph picks commits: everything between the two clicks, in
      // one hunk, since a part of a hunk is what git is given to stage.
      const lines = hunks[hunk]?.lines ?? [];
      for (let i = Math.min(from.line, line); i <= Math.max(from.line, line); i++) {
        const kind = lines[i]?.kind;
        if (kind !== undefined && pickable(kind)) next.add(`${hunk}:${i}`);
      }
    } else {
      const key = `${hunk}:${line}`;
      if (!next.delete(key)) next.add(key);
    }
    anchor = { hunk, line };
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
    if (diff.mode !== 'split' || !diff.readWhole || el === null || rows.length === 0 || placed === key) return;
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

  /** git's own mark for an untracked directory it has not looked inside: a trailing slash. */
  const isDirectory = $derived(diff.path?.endsWith('/') ?? false);

  /** The part of the file on screen, drawn over the marks so the strip says where you are. */
  const here = $derived(onScreen(split.rows.length, ROW, scrolled, viewport));

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

  /** Whether there is anything to step to, which decides the buttons' state in both layouts. */
  const anyChange = $derived(
    (diff.file?.hunks ?? []).some((h) => h.lines.some((l) => l.kind !== 'context')),
  );

  /** The unified table, so stepping can find the rows it has rendered. */
  let table: HTMLElement | null = $state(null);

  /**
   * Moves to the change before or after the one on screen, in the unified layout.
   *
   * By the rows themselves rather than by arithmetic: this table is not virtualised and its
   * rows are not a uniform height once a line wraps, so where a change actually sits is a
   * question only the rendered row can answer. A change is a run of added and removed lines,
   * and the step goes to the first line of the next run, set a third of the way down as side by
   * side sets it: centred, it sat below the line it was measured from and the next press found
   * the same row again.
   */
  function stepInline(direction: 1 | -1) {
    const root = table;
    const el = scroller;
    if (root === null || el === null) return;
    const all = [...root.querySelectorAll('tr')] as HTMLElement[];
    const changed = (row: HTMLElement | undefined) =>
      row !== undefined && (row.classList.contains('add') || row.classList.contains('remove'));
    const starts = all.filter((row, i) => changed(row) && !changed(all[i - 1]));
    if (starts.length === 0) return;

    // A third of the way down the pane, matching the split view: a change half off the bottom
    // is the next one, not one already passed.
    const line = el.getBoundingClientRect().top + el.clientHeight / 3;
    const next =
      direction === 1
        ? starts.find((row) => row.getBoundingClientRect().top > line + 1)
        : [...starts].reverse().find((row) => row.getBoundingClientRect().top < line - 1);
    if (next === undefined) return;
    el.scrollTop += next.getBoundingClientRect().top - line;
  }

  /** A file's text as lines, without the empty one after its last newline. */
  function linesOf(text: string): string[] {
    const lines = text.split('\n');
    if (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
    return lines;
  }

  /** The file's lines paired with the commit that last changed each. */
  const blamed = $derived.by(() => {
    const blame = diff.blame;
    const text = diff.text;
    if (blame === null || text === null) return [];

    const by = new Map<number, (typeof blame.chunks)[number]>();
    for (const chunk of blame.chunks) by.set(chunk.finalLine, chunk);

    return linesOf(text).map((content, i) => {
      const chunk = by.get(i + 1);
      const commit = chunk === undefined ? undefined : blame.commits[chunk.oid];
      return {
        no: i + 1,
        content,
        // Only the first line of a run carries the chip; a column repeating one name down
        // twenty lines is what makes a blame unreadable.
        oid: chunk?.oid ?? null,
        orig: chunk?.origLine ?? 0,
        who: commit?.author.name ?? '',
        when: commit?.author.time ?? 0,
        summary: commit?.summary ?? '',
      };
    });
  });

  /** The file view's lines: the file alone, with nobody beside them. */
  const fileLines = $derived.by(() => {
    const contents = diff.contents;
    if (contents?.kind !== 'text') return [];
    return linesOf(contents.text).map((content, i) => ({
      no: i + 1, content, oid: null, orig: 0, who: '', when: 0, summary: '',
    }));
  });

  /** Colours for the sheet's lines, which are the file's own and numbered as it is. */
  const sheetColours = $derived(
    diff.view === 'blame'
      ? (syntax?.tokens(grammar, diff.text) ?? null)
      : diff.view === 'file' && diff.contents?.kind === 'text'
        ? (syntax?.tokens(grammar, diff.contents.text) ?? null)
        : null,
  );

  /** Whether the scroller holds the file a line at a time rather than a diff. */
  const sheeted = $derived(diff.view === 'blame' || diff.view === 'file');
  const sheetLines = $derived(diff.view === 'blame' ? blamed : diff.view === 'file' ? fileLines : []);

  /** Height of the picked-lines bar above a blame, matching `.line-range` below. */
  const RANGE_BAR = 30;

  /**
   * Only the lines on screen are built, as side by side builds only its rows.
   *
   * Blame drew every line of the file as a table row, and the kernel's MAINTAINERS is thirty
   * thousand of them: a hundred thousand cells laid out before the first paint, for the forty
   * lines anyone can see. Measured from under the picked-lines bar, which blame keeps at the
   * top of the same scroller.
   */
  const firstLine = $derived(
    Math.max(0, Math.floor(Math.max(0, scrolled - (diff.view === 'blame' ? RANGE_BAR : 0)) / ROW) - OVERSCAN),
  );
  const shownLines = $derived(
    sheetLines.slice(firstLine, firstLine + Math.ceil(viewport / ROW) + OVERSCAN * 2),
  );

  /**
   * The columns of the sheet, sized from the whole file rather than from the lines built.
   *
   * With only some lines in the document, a column fitted to its content would change width as
   * longer names or lines scrolled into it, and the code would step sideways under the reader.
   * The longest line is counted with tabs at the eight columns `white-space: pre` gives them,
   * so the sheet is as wide as its widest line and scrolls sideways to it.
   */
  const sheetDigits = $derived(Math.max(3, String(sheetLines.length).length));
  const widest = $derived(sheetLines.reduce((n, line) => Math.max(n, columnsOf(line.content)), 0));
  const authorChars = $derived(
    Math.min(24, Object.values(diff.blame?.commits ?? {}).reduce((n, c) => Math.max(n, c.author.name.length), 0)),
  );

  function columnsOf(text: string): number {
    let n = 0;
    for (const c of text) n = c === '\t' ? n + 8 - (n % 8) : n + 1;
    return n;
  }

  /**
   * The run of blame lines picked for a history of their own, by clicking a number and
   * Shift-clicking another. Numbered as blame numbers them, which is the file at the revision
   * being shown, and that is what the history is read against.
   */
  let range = $state<{ from: number; to: number } | null>(null);
  let rangeAnchor = 0;

  function pickLine(no: number, event: MouseEvent) {
    if (event.shiftKey && range !== null) {
      range = { from: Math.min(rangeAnchor, no), to: Math.max(rangeAnchor, no) };
      return;
    }
    if (range !== null && range.from === no && range.to === no) {
      range = null;
      return;
    }
    rangeAnchor = no;
    range = { from: no, to: no };
  }

  /** The menu a blame chip opens, where it was clicked. */
  let chipMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  /**
   * What a commit named in a blame offers: going to it, blaming the file as it was before it,
   * and its id.
   *
   * Before it is where git says its lines came from, under the name the file had there; a
   * boundary — a root commit, or the edge of a shallow clone — has nothing before it to blame.
   */
  function openChip(event: MouseEvent, oid: string, line: number, orig: number) {
    const commit = diff.blame?.commits[oid];
    const previous = commit?.previous ?? null;
    const items: MenuItem[] = [];
    if (onShowCommit !== null) {
      items.push({ kind: 'item', label: 'Show commit in graph', run: () => onShowCommit(oid) });
    }
    items.push({
      kind: 'item',
      label: 'Blame before this commit',
      hint: previous === null ? 'nothing before it' : undefined,
      disabled: previous === null,
      run: () => {
        if (previous === null) return;
        void diff.blameBefore({
          rev: previous.oid, path: previous.filename, past: commit?.summary ?? oid.slice(0, 7), line: orig, back: line,
        });
      },
    });
    items.push({ kind: 'separator' });
    items.push({ kind: 'item', label: 'Copy commit id', run: () => void copyText(oid) });
    chipMenu = { x: event.clientX, y: event.clientY, items };
  }

  /**
   * Brings the reader's line into view once a blame reached by going back, or coming back,
   * lands: the same line is somewhere else in another revision of the file.
   */
  let placedBlame: unknown = null;
  $effect(() => {
    const blame = diff.blame;
    const line = diff.blameLine;
    const el = scroller;
    if (blame === null || line === null || el === null || placedBlame === blame) return;
    placedBlame = blame;
    el.scrollTop = Math.max(0, RANGE_BAR + (line - 1) * ROW - el.clientHeight / 3);
  });

  /** A new file, or a new revision of it, numbers its lines afresh. */
  $effect(() => {
    void diff.blame;
    range = null;
  });

  /** A hunk's new side as text: what the index will hold for it once it is staged. */
  function newSide(hunk: { lines: { kind: string; text: string }[] }): string {
    return hunk.lines.filter((l) => l.kind !== 'remove').map((l) => `${l.text}\n`).join('');
  }

  function linesLabel(from: number, to: number): string {
    return from === to ? `line ${from}` : `lines ${from}–${to}`;
  }

  function jump(event: MouseEvent) {
    const el = scroller;
    if (el === null) return;
    const strip = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (event.clientY - strip.top) / strip.height));
    const height = split.rows.length * ROW;
    el.scrollTop = Math.max(0, fraction * height - el.clientHeight / 2);
  }

</script>

{#snippet gapLineRow(line: Line)}
  <tr class="context opened">
    <td class="no">{line.oldNo}</td>
    <td class="no">{line.newNo}</td>
    <td class="text"
      ><span class="sign"> </span><CodeLine text={line.text} spans={null} tokens={coloursOf(line, 'after')} /></td
    >
  </tr>
{/snippet}

{#snippet gapRows(gap: Gap)}
  {@const now = opened[gap.below] ?? { top: 0, bottom: 0 }}
  {@const hidden = Math.max(0, gap.last - gap.first + 1 - now.top - now.bottom)}
  {#each numbers(gap.first, hidden === 0 ? gap.last : gap.first + now.top - 1) as no (no)}
    {@render gapLineRow(gapLine(no, gap.shift))}
  {/each}
  {#if hidden > 0}
    <tr class="gap">
      <td colspan="3">
        <div class="gap-bar">
          {#if gap.below > 0 && hidden > STEP}
            <button onclick={() => openGap(gap, 'top')} title="Show {STEP} more lines under the change above"
              ><Icon name="chevronDown" size={12} /> {STEP} more</button
            >
          {/if}
          <button onclick={() => openGap(gap, 'all')} title="Show every unchanged line here"
            >{hidden === 1 ? 'The 1 unchanged line' : `All ${hidden.toLocaleString()} unchanged lines`}</button
          >
          {#if gap.below < hunks.length && hidden > STEP}
            <button onclick={() => openGap(gap, 'bottom')} title="Show {STEP} more lines over the change below"
              ><Icon name="chevronUp" size={12} /> {STEP} more</button
            >
          {/if}
        </div>
      </td>
    </tr>
    {#each numbers(gap.last - now.bottom + 1, gap.last) as no (no)}
      {@render gapLineRow(gapLine(no, gap.shift))}
    {/each}
  {/if}
{/snippet}

{#snippet sheet(blaming: boolean)}
  <div
    class="file-sheet"
    class:blame={blaming}
    style:height="{sheetLines.length * ROW}px"
    style:--gutter="calc({sheetDigits}ch + var(--space-3) + var(--space-2))"
    style:--aside={blaming ? `calc(${authorChars + 12}ch + 3 * var(--space-2))` : '0px'}
    style:min-width="calc(var(--aside) + var(--gutter) + {widest}ch + 2 * var(--space-2))"
  >
    <div class="window" style:transform="translateY({firstLine * ROW}px)">
      {#each shownLines as line (line.no)}
        <div class="file-line" class:ranged={blaming && range !== null && line.no >= range.from && line.no <= range.to}>
          {#if blaming}
            <span class="author">
              {#if line.oid !== null}
                {@const oid = line.oid}
                <button
                  class="chip"
                  title="{line.summary}&#10;{line.who}"
                  onclick={(e) => openChip(e, oid, line.no, line.orig)}
                >
                  <span class="name">{line.who}</span>
                  <span class="mono">{line.oid.slice(0, 7)}</span>
                  <span class="when">{shortAge(line.when)}</span>
                </button>
              {/if}
            </span>
            <button
              class="no line-pick"
              onclick={(e) => pickLine(line.no, e)}
              title="Pick this line; Shift-click another to pick the lines between"
            >{line.no}</button>
          {:else}
            <span class="no">{line.no}</span>
          {/if}
          <span class="text"><CodeLine text={line.content} spans={null} tokens={sheetColours?.[line.no - 1] ?? null} /></span>
        </div>
      {/each}
    </div>
  </div>
{/snippet}

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
        <!-- Blame reads the file as lines, which a binary has none of: git answers for one all
             the same and the pane painted four kilobytes of replacement characters. -->
        <button
          class:on={diff.view === 'blame'}
          disabled={diff.file?.binary === true}
          title={diff.file?.binary === true ? 'A binary file has no lines to blame' : 'Blame'}
          onclick={() => diff.setView('blame')}>Blame</button
        >
        <button class:on={diff.view === 'history'} onclick={() => diff.setView('history')}>
          History
        </button>
        <button
          class:on={diff.view === 'file'}
          disabled={diff.file?.binary === true}
          title={diff.file?.binary === true ? 'A binary file has no lines to show' : 'The whole file as this change leaves it'}
          onclick={() => diff.setView('file')}>File</button
        >
      </div>
    {/if}

    <!--
      The free space sits here rather than in the file name, so that what follows is pushed to
      the right and what precedes it stays put. Everything after this comes and goes with the
      view: choosing Blame took the layout toggle away and slid the three tabs a quarter of the
      header to the right, out from under the pointer that had just chosen one of them.
    -->
    <span class="spread"></span>

    {#if diff.view === 'file' && diff.contents !== null && diff.contents.kind !== 'absent' && diff.contents.before}
      <span class="note">As it was: this change deleted it</span>
    {/if}

    {#if diff.view === 'blame'}
      {@const step = diff.blameTrail.at(-1)}
      {@const skipping = diff.blame?.skipping}
      {#if step !== undefined}
        <!-- The file blamed is named when it is not the one in the header: going back past a
             rename blames the file under the name it had then. -->
        <span class="note" title={step.path}
          >{step.path === diff.path ? 'Before' : `${step.path}, before`} “{step.past}”</span
        >
        <button class="wider" onclick={() => void diff.blameBack()} title="Back to the blame this came from">Back</button>
      {/if}
      {#if skipping?.kind === 'listed'}
        <button
          class="wider"
          onclick={() => void diff.setSkipListed(false)}
          title="Looking past the commits .git-blame-ignore-revs lists, reformats mostly. Click to name them too."
        >Skipping {skipping.count} listed</button>
      {:else if skipping?.kind === 'offered'}
        <button
          class="wider"
          onclick={() => void diff.setSkipListed(true)}
          title="Look past the commits .git-blame-ignore-revs lists, reformats mostly"
        >Skip {skipping.count} listed</button>
      {:else if skipping?.kind === 'configured'}
        <span class="note" title="blame.ignoreRevsFile in your git configuration">Skipping what {skipping.file} lists</span>
      {:else if skipping?.kind === 'missing'}
        <span class="note warn" title="blame.ignoreRevsFile in your git configuration"
          >{skipping.file} is not there, so it skips nothing</span
        >
      {/if}
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

      {#if diff.mode === 'inline' && diff.file && !diff.file.binary}
        <button
          class="wider"
          class:on={diff.expanded}
          onclick={() => diff.setExpanded(!diff.expanded)}
          title={diff.expanded
            ? 'Show only the changed parts of the file'
            : 'Show the whole file around the changes'}
        >
          {diff.expanded ? 'Changes only' : 'Whole file'}
        </button>
      {/if}

      {#if diff.mode === 'split'}
        <div class="steps">
          <button
            onclick={() => step(-1)}
            disabled={changeRows.length === 0}
            title="Previous change"><Icon name="chevronUp" size={14} /></button
          >
          <button
            onclick={() => step(1)}
            disabled={changeRows.length === 0}
            title="Next change"><Icon name="chevronDown" size={14} /></button
          >
        </div>
      {:else if diff.mode === 'inline'}
        <div class="steps">
          <button onclick={() => stepInline(-1)} disabled={!anyChange} title="Previous change"
            ><Icon name="chevronUp" size={14} /></button
          >
          <button onclick={() => stepInline(1)} disabled={!anyChange} title="Next change"
            ><Icon name="chevronDown" size={14} /></button
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
      ><Icon name="pilcrow" size={14} /></button>
    {/if}
    <button class="close" onclick={onClose} aria-label="Close the diff"><Icon name="close" size={15} /></button>
  </header>

  <div class="body">
  {#if diff.view === 'history'}
    <!--
      The commits that touched this file, newest first. Selecting one shows what it did to
      the file, which is the question a file history is opened to answer.
    -->
    <ul class="history">
      {#if diff.lines !== null}
        <li class="scope">
          <span>History of {linesLabel(diff.lines.from, diff.lines.to)}</span>
          <button onclick={() => void diff.historyOfFile()} title="The history of the whole file again">
            Whole file
          </button>
        </li>
      {/if}
      {#each diff.history ?? [] as commit (commit.oid)}
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
      {#if diff.sideError}
        <li class="error">{diff.sideError}</li>
      {:else if diff.history === null}
        <li class="muted">
          {diff.lines === null ? 'Reading what has touched this file…' : 'Reading what has changed these lines…'}
        </li>
      {:else if diff.history.length === 0}
        <li class="muted">
          {diff.lines === null ? 'Nothing has touched this file.' : 'Nothing has changed these lines.'}
        </li>
      {:else if diff.moreHistory}
        <li>
          <button class="deeper" onclick={() => void diff.deeper()}>Go deeper</button>
        </li>
      {/if}
    </ul>
  {/if}

  <div class="scroll" bind:this={scroller} onscroll={onScroll} bind:clientHeight={viewport}>
    {#if diff.view === 'blame'}
      {#if diff.sideError}
        <p class="error">{diff.sideError}</p>
      {:else if diff.blame === null || diff.text === null}
        <p class="muted">Working out who wrote each line…</p>
      {:else}
        <!-- Always there, so picking a line moves nothing: a bar that appeared on the first
             pick pushed every row down by its height, and the second click landed on the line
             just picked and let it go. Sticky, so the way on is in view wherever it was made. -->
        <div class="line-range" role="status">
          {#if range === null}
            <span class="muted">Pick a line number for the history of that line; Shift-click another for the lines between.</span>
          {:else}
            <span>{linesLabel(range.from, range.to)} picked</span>
            <button
              class="go"
              onclick={() => {
                if (range !== null) void diff.historyOfLines(range.from, range.to);
              }}
            >History of {range.from === range.to ? 'this line' : 'these lines'}</button>
            <button onclick={() => (range = null)} aria-label="Clear the picked lines">
              <Icon name="close" size={12} />
            </button>
          {/if}
        </div>
        {@render sheet(true)}
      {/if}
    {:else if diff.view === 'file'}
      {#if diff.sideError}
        <p class="error">{diff.sideError}</p>
      {:else if diff.contents === null}
        <p class="muted">Reading the file…</p>
      {:else if diff.contents.kind === 'absent'}
        <p class="muted">There is no file here to show.</p>
      {:else if diff.contents.kind === 'binary'}
        <p class="muted">A binary file has no lines to show.</p>
      {:else if fileLines.length === 0}
        <p class="muted">The file is empty.</p>
      {:else}
        {@render sheet(false)}
      {/if}
    {:else if diff.loading}
      <p class="muted">Loading…</p>
    {:else if diff.error}
      <p class="error">{diff.error}</p>
    {:else if diff.empty}
      <!-- Not red: the file is simply not part of this change, which is an answer. The file
           itself is still there to read, which is what a click in "view all files" is after. -->
      <p class="muted">
        {diff.empty}
        {#if diff.source !== 'compare'}
          <button class="anyway" onclick={() => diff.setView('file')}>Show the file</button>
        {/if}
      </p>
    {:else if !diff.file}
      <p class="muted">Nothing to show.</p>
    {:else if diff.file.binary}
      {@const sides = diff.sides}
      {#if sides !== null && isImage(sides.after.file)}
        <ImageDiff repo={sides.repo} before={sides.before} after={sides.after} stamp={diff.file} />
      {:else}
        <p class="muted">Binary file — no textual diff.</p>
      {/if}
    {:else if diff.file.tooLarge}
      <!--
        The guard is there because one generated file the size of a kernel header dump costs a
        visible pause to parse, not because the change is unreadable. Saying only that the
        contents were not read left no way to ever see such a file.
      -->
      <p class="muted">
        The file is past the size guard, so its contents were not read.
        <button class="anyway" onclick={() => void diff.readAnyway()}>Read it anyway</button>
      </p>
    {:else if isDirectory}
      <!--
        git collapses an untracked directory to one entry — `? notes/` — rather than listing
        what is inside it, which is what keeps `status` fast on a repository with a build tree
        in it. There is no diff to read for that entry, and answering "No line changes." said
        the opposite of what is true of a directory full of new files.
      -->
      <p class="muted">
        A directory of new files. git lists one that is not tracked as a single entry, so
        there is nothing here to read line by line; staging it adds everything inside.
      </p>
    {:else if diff.file.hunks.length === 0}
      <!--
        A mode-only change has no hunks at all, so this was the whole of what a file listed as
        modified had to say for itself: "No line changes." about a file that had changed.
      -->
      <p class="muted">
        {emptyDiffSays(diff.file)}
        {#if diff.file.mode}
          <span class="modes"
            >The file mode went from <code>{diff.file.mode.old}</code> to
            <code>{diff.file.mode.new}</code>.</span
          >
        {/if}
      </p>
    {:else if diff.mode === 'inline'}
      <table class="lines" bind:this={table}>
        <tbody>
          {#each hunks as hunk, h (hunk.header + hunk.newStart)}
            {@const gap = gaps.get(h)}
            {#if gap !== undefined}{@render gapRows(gap)}{/if}
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
                      <button
                        onclick={() => onEdit(h, newSide(hunk))}
                        title="Stage this hunk as you rewrite it, leaving the file on disk alone"
                      >Edit…</button>
                      <button class="risky" onclick={() => onPart('discard', h, pickedIn(h))}>
                        Discard{pickedIn(h).length > 0 ? ' lines' : ' hunk'}
                      </button>
                    {/if}
                  </span>
                {:else if diff.atCommit === null && diff.ignoreWhitespace && (diff.source === 'unstaged' || diff.source === 'staged')}
                  <span class="hunk-note">Show whitespace to stage or discard part of this</span>
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
                    <button class="pick" onclick={(e) => togglePick(e, h, i)} title="Pick this line; Shift picks a run"
                      ><span class="sign">{sign[line.kind]}</span><CodeLine
                        text={line.text}
                        spans={words.get(line) ?? null}
                        tokens={coloursOf(line, line.kind === 'remove' ? 'before' : 'after')}
                      />{#if line.noNewline}<span class="nonl" title={NO_NEWLINE}
                        >no newline</span
                      >{/if}</button
                    >
                  </td>
                {:else}
                  <td class="text"
                    ><span class="sign">{sign[line.kind]}</span><CodeLine
                      text={line.text}
                      spans={words.get(line) ?? null}
                      tokens={coloursOf(line, line.kind === 'remove' ? 'before' : 'after')}
                    />{#if line.noNewline}<span class="nonl" title={NO_NEWLINE}>no newline</span
                      >{/if}</td
                  >
                {/if}
              </tr>
            {/each}
          {/each}
          {#if trailingGap !== undefined}{@render gapRows(trailingGap)}{/if}
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
              <span class="cell {row.left ? row.left.kind : 'blank'}"
                ><CodeLine
                  text={row.left?.text ?? ''}
                  spans={row.left ? words.get(row.left) ?? null : null}
                  tokens={coloursOf(row.left ?? null, 'before')}
                />{#if row.left?.noNewline}<span class="nonl" title={NO_NEWLINE}>no newline</span
                  >{/if}</span
              >
              <span class="no">{row.right?.newNo ?? ''}</span>
              <span class="cell {row.right ? row.right.kind : 'blank'}"
                ><CodeLine
                  text={row.right?.text ?? ''}
                  spans={row.right ? words.get(row.right) ?? null : null}
                  tokens={coloursOf(row.right ?? null, 'after')}
                />{#if row.right?.noNewline}<span class="nonl" title={NO_NEWLINE}>no newline</span
                  >{/if}</span
              >
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if sheeted}
      <!-- The sheet is the whole file, so there is no window of it to explain. -->
    {:else if diff.mode === 'split' && split.total > split.rows.length}
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

  <!-- Not in blame or the file: the marks, the band and the jump below all measure against the
       diff's rows, and those put the whole file in this same scroller instead. -->
  {#if diff.mode === 'split' && !sheeted && marks.length > 0}
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
      {#if here !== null}
        <span class="here" style:top="{here.at * 100}%" style:height="{here.size * 100}%"></span>
      {/if}
    </button>
  {/if}
  </div>
</section>

{#if chipMenu !== null}
  <Menu x={chipMenu.x} y={chipMenu.y} items={chipMenu.items} onClose={() => (chipMenu = null)} />
{/if}

<style>
  .diff {
    flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--bg-0);
    border: var(--hairline) solid var(--border); border-radius: var(--radius-3); overflow: hidden;
  }
  header {
    display: flex; align-items: center; gap: var(--space-2);
    height: 36px; padding: 0 var(--space-2) 0 var(--space-3); flex: 0 0 auto;
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
    flex: 0 1 auto; min-width: 3em; font-size: var(--text-base); font-weight: var(--weight-regular);
    color: var(--fg-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .spread { flex: 1 1 auto; min-width: var(--space-2); }
  .tally {
    flex: 0 0 auto; font-size: var(--text-sm); display: flex; gap: var(--space-2);
    font-family: var(--font-mono); font-variant-ligatures: none; font-variant-numeric: tabular-nums;
  }
  .added { color: var(--ok); }
  .removed { color: var(--danger); }

  /*
   * One shape for every control in this header.
   *
   * There were three: a bordered group of segments, a lone bordered button, and a pair of
   * arrows in boxes of their own with a third radius. Six controls, four outlines and three
   * corner radii across thirty-four pixels.
   *
   * Segments now sit in a filled track with the chosen one lifted out of it, which is what
   * says a set of choices rather than a row of buttons; everything else is a plain button
   * with no outline until the pointer is on it.
   */
  .toggle {
    display: flex; gap: 2px; flex: 0 0 auto;
    padding: 2px; border-radius: var(--radius-1); background: var(--bg-2);
  }
  .toggle button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 2px var(--space-2);
    background: none; border: 0; border-radius: var(--radius-0); color: var(--fg-2);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on {
    background: var(--bg-0); color: var(--fg-0); font-weight: var(--weight-semibold);
    box-shadow: var(--elevate-1);
  }
  .close {
    display: flex; align-items: center; justify-content: center; flex: 0 0 auto;
    width: 24px; height: 24px; margin-left: var(--space-1);
    font: inherit; cursor: pointer; background: none; border: 0; color: var(--fg-2);
    border-radius: var(--radius-1);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .close:hover { color: var(--fg-0); background: var(--bg-2); }

  .body { flex: 1; min-height: 0; display: flex; }

  .steps { display: flex; gap: 2px; flex: 0 0 auto; }
  .steps button, .ws, .wider {
    display: flex; align-items: center; justify-content: center;
    font: inherit; font-size: var(--text-sm); cursor: pointer;
    min-width: 24px; height: 24px; padding: 0 var(--space-2);
    background: none; border: 0; border-radius: var(--radius-1);
    color: var(--fg-2);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .steps button { padding: 0; }
  .steps button:hover:not(:disabled), .ws:hover, .wider:hover {
    background: var(--bg-2); color: var(--fg-0);
  }
  .steps button:disabled { opacity: 0.4; cursor: default; }
  /* A pressed toggle keeps the tinted fill rather than the solid accent: these sit beside the
     segmented tracks, and two different ways of saying "chosen" in one header is one too
     many. */
  .wider.on, .ws.on { background: var(--accent-soft); color: var(--accent); font-weight: var(--weight-semibold); }

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
    font: inherit; font-size: var(--text-base); cursor: pointer; color: var(--fg-1);
    background: var(--bg-1); border: 0; border-radius: var(--radius-1);
    padding: var(--space-1) var(--space-2); overflow: hidden;
  }
  .entry:hover { background: var(--bg-2); }
  .entry.on { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line); }
  .who {
    flex: 0 0 auto; width: 24px; height: 24px; border-radius: var(--radius-1);
    display: flex; align-items: center; justify-content: center;
    /* White, and this is the one place a literal is right: the node colours are
       deliberately dark in both themes so the letters inside a commit node can be white in
       both. */
    background: var(--node-1); color: #ffffff; font-size: var(--text-xs); font-weight: var(--weight-semibold);
  }
  .what { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .subject { color: var(--fg-0); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .by { color: var(--fg-2); font-size: var(--text-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sha { flex: 0 0 auto; color: var(--fg-2); font-size: var(--text-sm); }
  .deeper {
    width: 100%; font: inherit; font-size: var(--text-sm); cursor: pointer; margin-top: var(--space-2);
    background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    color: var(--fg-1); padding: var(--space-1);
  }
  .deeper:hover { background: var(--bg-3); color: var(--fg-0); }
  .scope {
    display: flex; align-items: center; justify-content: space-between; gap: var(--space-2);
    padding: var(--space-1) var(--space-2) var(--space-2); color: var(--fg-1); font-size: var(--text-sm);
    border-bottom: 1px solid var(--border); margin-bottom: var(--space-1);
  }
  .scope button, .line-range button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; white-space: nowrap;
    background: var(--bg-0); color: var(--fg-1); border: 1px solid var(--border);
    border-radius: var(--radius-0); padding: 0 var(--space-2); line-height: 18px;
  }
  .scope button:hover, .line-range button:hover { background: var(--bg-2); color: var(--fg-0); }
  /* Held at the left as well as the top: the sheet under it is as wide as the file's widest
     line and scrolls sideways, and the way on should not scroll out of view with it. */
  .line-range {
    position: sticky; top: 0; left: 0; z-index: 2; display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-1) var(--space-2); background: var(--bg-1);
    border-bottom: 1px solid var(--border); color: var(--fg-1); font-size: var(--text-sm);
  }
  .line-range .go { color: var(--fg-0); border-color: var(--accent); }
  /* One height whatever it says, and the hint kept to one line: the bar is there so that
     picking a line moves nothing, which a hint taller than the picked state undid. */
  .line-range { box-sizing: border-box; height: 30px; flex-wrap: nowrap; }
  .line-range > span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .line-range .muted { color: var(--fg-2); margin: 0; padding: 0; }
  .file-line.ranged .no { background: var(--accent-soft); color: var(--fg-0); box-shadow: inset 2px 0 0 var(--accent-line); }
  .file-line.ranged .text { background: var(--accent-soft); }

  .modes code {
    font-family: var(--font-mono); font-size: var(--text-sm);
    background: var(--bg-2); border-radius: var(--radius-1); padding: 0 4px;
  }

  /* Set apart from the line it is about: it is git's note, not part of the file. */
  .nonl {
    margin-left: var(--space-2); padding: 0 4px; border-radius: var(--radius-1);
    background: var(--bg-2); color: var(--fg-2); font-size: var(--text-sm);
    user-select: none; white-space: nowrap;
  }

  /* Beside the sentence it answers, not across the panel: `.deeper` is sized for the narrow
     history column and here it would be a button the width of the diff. */
  .anyway {
    font: inherit; font-size: var(--text-sm); cursor: pointer; margin-left: var(--space-2);
    background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    color: var(--fg-1); padding: var(--space-1) var(--space-2);
  }
  .anyway:hover { background: var(--bg-3); color: var(--fg-0); }

  /*
   * Blame and the file, a line at a time: a spacer as tall as the file with only the lines on
   * screen inside it, as side by side is drawn. The window is the same rule as that sheet's.
   */
  .file-sheet {
    position: relative;
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); font-weight: var(--weight-regular); line-height: 17px;
  }
  .file-line { display: grid; height: 17px; grid-template-columns: var(--gutter) minmax(0, 1fr); }
  .file-sheet.blame .file-line { grid-template-columns: var(--aside) var(--gutter) minmax(0, 1fr); }
  .file-line .no {
    text-align: right; user-select: none; padding: 0 var(--space-2) 0 var(--space-3);
    color: var(--fg-2); background: var(--bg-2); box-sizing: border-box;
    border: 0; border-right: 1px solid var(--border);
    font: inherit; font-variant-numeric: tabular-nums;
  }
  button.no.line-pick { cursor: pointer; }
  button.no.line-pick:hover { color: var(--fg-0); }

  /* Who wrote each line, named once per run rather than once per line. */
  .blame .author {
    min-width: 0; white-space: nowrap; user-select: none;
    padding: 0 var(--space-2); background: var(--bg-1);
    border-right: 1px solid var(--border);
  }
  .chip {
    display: flex; gap: var(--space-2); align-items: baseline; min-width: 0; width: 100%;
    font-family: var(--font-ui); font-size: var(--text-sm); color: var(--fg-1);
    background: none; border: 0; padding: 0; cursor: pointer; text-align: left; line-height: 17px;
  }
  .chip:hover .name { color: var(--fg-0); text-decoration: underline; }
  .note.warn { color: var(--warn); }
  .chip .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .chip .mono, .chip .when { flex: 0 0 auto; }
  .chip .when { color: var(--fg-2); }
  .note { flex: 0 1 auto; min-width: 0; color: var(--fg-2); font-size: var(--text-sm); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; background: var(--bg-1); }
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
  .mark { position: absolute; left: 2px; right: 2px; border-radius: var(--radius-0); }
  .mark.add { background: var(--ok); }
  .mark.remove { background: var(--danger); }
  /*
   * A run that both removed and added lines is a modification, so it is drawn in the two
   * colours it is made of rather than in a third. That third was `--lane-3`, a graph lane
   * colour: mustard here, and lane three everywhere else in the window, which is a palette
   * saying two things with one colour. Split across the strip rather than down it, so a run
   * of two pixels still shows both halves.
   */
  .mark.both {
    background: linear-gradient(to right, var(--danger) 0 50%, var(--ok) 50% 100%);
  }
  /* An outline rather than a fill: the marks under it are the point of the strip. */
  .here {
    position: absolute; left: 0; right: 0; min-height: 8px;
    border: 1px solid var(--fg-2); border-radius: var(--radius-0); background: none;
  }
  .lines {
    border-collapse: collapse; width: 100%;
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); font-weight: var(--weight-regular);
    line-height: 17px;
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
    border-right: 1px solid var(--border);
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
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); font-weight: var(--weight-regular); line-height: 17px;
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
    border-right: 1px solid var(--border); box-sizing: border-box;
    font-variant-numeric: tabular-nums;
  }
  /*
   * Clipped, and saying so. The rows are a fixed height so the sheet can hold a file of any
   * length, which rules out wrapping, and the columns are capped so a long line cannot push
   * the other pane off the window. That leaves the text cut at the column edge, and without
   * the ellipsis nothing said it had been: a five-thousand-character line looked like a line
   * that ended there. The inline view scrolls sideways and is where such a line is read.
   */
  .cell {
    white-space: pre; overflow: hidden; text-overflow: ellipsis; padding: 0 var(--space-2);
    color: var(--fg-0); background: var(--bg-0);
  }
  .cell.add { background: var(--add-bg); box-shadow: inset 2px 0 0 var(--ok); --word-mark: var(--add-word); }
  .cell.remove { background: var(--remove-bg); box-shadow: inset 2px 0 0 var(--danger); --word-mark: var(--remove-word); }
  .cell.blank { background: var(--bg-1); }
  .sign { user-select: none; color: var(--fg-2); }
  tr.add .text { background: var(--add-bg); box-shadow: inset 2px 0 0 var(--ok); --word-mark: var(--add-word); }
  tr.remove .text { background: var(--remove-bg); box-shadow: inset 2px 0 0 var(--danger); --word-mark: var(--remove-word); }
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
  .hunk-note { flex: 0 0 auto; font-family: var(--font-ui); color: var(--fg-2); }
  .hunk-acts button {
    font: inherit; font-family: var(--font-ui); font-size: var(--text-sm); cursor: pointer;
    background: var(--bg-0); border: 1px solid var(--border); border-radius: var(--radius-0);
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
  /* The row's own colour, deepened, so a picked line still reads as added or removed. An
     accent laid over it turned a dark theme's removed line grey, and the thicker stripe alone,
     before that, was the same green as an added line's. */
  tr.picked.add .text {
    background-color: color-mix(in srgb, var(--ok) 28%, var(--bg-0));
    box-shadow: inset 4px 0 0 var(--accent);
  }
  tr.picked.remove .text {
    background-color: color-mix(in srgb, var(--danger) 28%, var(--bg-0));
    box-shadow: inset 4px 0 0 var(--accent);
  }
  tr.picked .sign { font-weight: var(--weight-bold); }

  /* The way into a gap: quieter than a hunk's header, since it is not a change. */
  tr.gap td { padding: 2px var(--space-2); background: var(--bg-1); border-top: 1px solid var(--border); border-bottom: 1px solid var(--border); }
  .gap-bar { display: flex; gap: var(--space-2); align-items: center; font-family: var(--font-ui); }
  .gap-bar button {
    display: inline-flex; align-items: center; gap: 4px;
    font: inherit; font-size: var(--text-sm); cursor: pointer; line-height: 17px;
    background: var(--bg-0); border: 1px solid var(--border); border-radius: var(--radius-0);
    color: var(--fg-1); padding: 0 6px;
  }
  .gap-bar button:hover { background: var(--bg-3); color: var(--fg-0); }

  tr.hunk td {
    background: var(--bg-2); color: var(--fg-2); padding: 3px var(--space-2);
    white-space: pre; user-select: none;
    border-top: 1px solid var(--border); border-bottom: 1px solid var(--border);
    font-size: var(--text-xs); letter-spacing: 0.02em;
  }
  .muted { color: var(--fg-2); padding: var(--space-3); font-size: var(--text-base); }
  .error { color: var(--danger); padding: var(--space-3); font-size: var(--text-base); }
</style>
