import type { Hunk, Line } from '../ipc/types';

/** A stretch of one line, marked where it differs from the line it replaced. */
export interface Span {
  text: string;
  marked: boolean;
}

/**
 * Words, runs of blank space, and single punctuation characters.
 *
 * The unit a reader compares. Marking character by character turns a renamed variable into
 * confetti — every letter the two names happen to share is left unmarked in the middle of the
 * one that changed, and the eye cannot put it back together.
 */
function words(text: string): string[] {
  return text.match(/[\p{L}\p{N}_]+|\s+|[^\p{L}\p{N}_\s]/gu) ?? [];
}

/**
 * How much of the shorter line has to survive before marking says anything.
 *
 * The question is not how much changed but whether these are the same line at all: two lines
 * with almost nothing in common were paired by their position in the run, not because one
 * became the other, and marking nine tenths of such a pair only repeats what the line's own
 * colour already said.
 *
 * Asked this way round rather than as "how much is marked", because a line that was only
 * added to fails that test and is exactly the case marking is best at: `line two` becoming
 * `line two, rewritten by hand` is seventy per cent new, and every word of the original is
 * still there to be seen.
 */
const ENOUGH_SURVIVES = 0.25;

/** And a floor in characters, for the short lines a fraction cannot speak for. */
const AT_LEAST = 3;

/** The largest table the token diff will build for one pair of lines. */
const PAIR_CELLS = 40_000;

/**
 * And for a whole file.
 *
 * A file where every line changed is thousands of pairs, and the table is quadratic in the
 * length of a line. This is the point past which the rest of the file is left unmarked rather
 * than the panel being late.
 */
const FILE_CELLS = 2_000_000;

/**
 * What changed inside one replaced line, on each side of it.
 *
 * Null where marking would say nothing: the two lines are the same, or so much of them differs
 * that the whole line is the answer.
 */
export function markedPair(before: string, after: string): { left: Span[]; right: Span[] } | null {
  if (before === after) return null;

  const a = words(before);
  const b = words(after);

  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head += 1;
  let tail = 0;
  while (
    tail < a.length - head &&
    tail < b.length - head &&
    a[a.length - 1 - tail] === b[b.length - 1 - tail]
  ) {
    tail += 1;
  }

  const midA = a.slice(head, a.length - tail);
  const midB = b.slice(head, b.length - tail);

  // Past the budget the middles are marked whole. They are already what is left after the
  // shared ends were trimmed, so this is a long line that was largely rewritten — and the
  // check below is about to throw the marking away anyway.
  const shared =
    midA.length * midB.length <= PAIR_CELLS
      ? common(midA, midB)
      : { a: new Array<boolean>(midA.length).fill(false), b: new Array<boolean>(midB.length).fill(false) };

  const left = spansOf(a, head, tail, shared.a);
  const right = spansOf(b, head, tail, shared.b);

  // Blank space is not evidence of anything: two lines that share only their indentation are
  // not the same line, however many spaces they begin with.
  const survives = keptLength(left);
  if (survives < AT_LEAST) return null;
  if (survives < Math.min(before.length, after.length) * ENOUGH_SURVIVES) return null;
  return { left, right };
}

/**
 * The marks in a file's changed lines, keyed by the line they belong to.
 *
 * Keyed by the line rather than by an index because the two layouts index differently: the
 * unified table walks a hunk's own lines and the side-by-side one walks rows built from them.
 * Both hold the same objects, so both can look a line up here.
 */
export function wordMarks(hunks: readonly Hunk[]): Map<Line, Span[]> {
  const marks = new Map<Line, Span[]>();
  let budget = FILE_CELLS;

  for (const hunk of hunks) {
    let removed: Line[] = [];
    let added: Line[] = [];

    const flush = () => {
      // A run with lines on one side only is an insertion or a deletion, and there is nothing
      // to compare it against. Only the lines that were replaced are marked.
      const pairs = Math.min(removed.length, added.length);
      for (let i = 0; i < pairs; i += 1) {
        const from = removed[i];
        const to = added[i];
        if (from === undefined || to === undefined || budget <= 0) continue;
        budget -= from.text.length * to.text.length;
        const pair = markedPair(from.text, to.text);
        if (pair === null) continue;
        marks.set(from, pair.left);
        marks.set(to, pair.right);
      }
      removed = [];
      added = [];
    };

    for (const line of hunk.lines) {
      if (line.kind === 'remove') removed.push(line);
      else if (line.kind === 'add') added.push(line);
      else flush();
    }
    flush();
  }

  return marks;
}

/**
 * Which tokens of each side the two have in common, longest run first.
 *
 * The whole point of doing this over tokens rather than characters: `sort_by` against
 * `sort_unstable_by` shares four letters in three places as characters, and one token — none —
 * as words.
 */
function common(a: string[], b: string[]): { a: boolean[]; b: boolean[] } {
  const n = a.length;
  const m = b.length;
  const width = m + 1;
  const table = new Int32Array((n + 1) * width);

  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      table[i * width + j] =
        a[i] === b[j]
          ? (table[(i + 1) * width + j + 1] ?? 0) + 1
          : Math.max(table[(i + 1) * width + j] ?? 0, table[i * width + j + 1] ?? 0);
    }
  }

  const keepA = new Array<boolean>(n).fill(false);
  const keepB = new Array<boolean>(m).fill(false);
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      keepA[i] = true;
      keepB[j] = true;
      i += 1;
      j += 1;
    } else if ((table[(i + 1) * width + j] ?? 0) >= (table[i * width + j + 1] ?? 0)) {
      i += 1;
    } else {
      j += 1;
    }
  }
  return { a: keepA, b: keepB };
}

/**
 * One side's tokens as spans, with the shared ends and the tokens the other side also has
 * left unmarked.
 *
 * Adjacent tokens with the same answer are joined, so a changed call is one mark rather than
 * one per identifier, bracket and comma.
 */
function spansOf(tokens: string[], head: number, tail: number, shared: boolean[]): Span[] {
  const out: Span[] = [];
  const middle = tokens.length - tail;

  for (let i = 0; i < tokens.length; i += 1) {
    const text = tokens[i] ?? '';
    const marked = i >= head && i < middle && !(shared[i - head] ?? false);
    const last = out[out.length - 1];
    if (last !== undefined && last.marked === marked) last.text += text;
    else out.push({ text, marked });
  }
  return out;
}

/** The printing characters both sides kept, which is what says they are the same line. */
function keptLength(spans: readonly Span[]): number {
  return spans.reduce(
    (n, span) => (span.marked ? n : n + span.text.replace(/\s+/gu, '').length),
    0,
  );
}
