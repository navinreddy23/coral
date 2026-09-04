import type { Hunk, Line } from '../ipc/types';

/** One row of a side-by-side view: the old file's line, the new file's, or both. */
export interface SplitRow {
  left: Line | null;
  right: Line | null;
}

/**
 * Pairs a hunk's removals with its additions for a side-by-side view.
 *
 * A unified hunk lists every removal in a run before the additions that replaced them, so the
 * two are paired positionally within each run: the first removal sits opposite the first
 * addition. Where a run is longer on one side the extra rows are left blank on the other,
 * which is what shows at a glance that lines were added rather than changed.
 */
export function splitRows(hunk: Hunk): SplitRow[] {
  const rows: SplitRow[] = [];
  let removed: Line[] = [];
  let added: Line[] = [];

  const flush = () => {
    const height = Math.max(removed.length, added.length);
    for (let i = 0; i < height; i++) {
      rows.push({ left: removed[i] ?? null, right: added[i] ?? null });
    }
    removed = [];
    added = [];
  };

  for (const line of hunk.lines) {
    if (line.kind === 'remove') removed.push(line);
    else if (line.kind === 'add') added.push(line);
    else {
      flush();
      rows.push({ left: line, right: line });
    }
  }
  flush();
  return rows;
}

/**
 * A window of at most `limit` rows that contains the first change.
 *
 * A whole file is more rows than the panel can hold as DOM, and every way of cutting it but
 * this one can show a file with no change in it — which is the one thing the reader opened it
 * for. The window starts a quarter of the budget above the first change, so there is context
 * over it rather than the change sitting on the first line.
 */
export function windowAround(
  rows: SplitRow[],
  limit: number,
): { rows: SplitRow[]; from: number } {
  if (rows.length <= limit) return { rows, from: 0 };

  const change = firstChangedRow(rows);
  if (change < 0) return { rows: rows.slice(0, limit), from: 0 };

  const lead = Math.floor(limit / 4);
  const from = Math.min(Math.max(0, change - lead), rows.length - limit);
  return { rows: rows.slice(from, from + limit), from };
}

/** True for a row that is not the same line on both sides. */
export function isChangedRow(row: SplitRow): boolean {
  return row.left?.kind !== 'context' || row.right?.kind !== 'context';
}

/** Where the first change is, or -1 when the two sides are identical throughout. */
export function firstChangedRow(rows: readonly SplitRow[]): number {
  return rows.findIndex(isChangedRow);
}

/** One run of changed rows, as a fraction of the file, for the strip beside the diff. */
export interface Mark {
  /** Where the run starts, 0 to 1. */
  at: number;
  /** How much of the file it covers, 0 to 1. */
  size: number;
  kind: 'add' | 'remove' | 'both';
}

/**
 * The changes in a file, as runs to mark on a strip beside it.
 *
 * A whole-file diff is mostly unchanged text, and the scrollbar says nothing about where the
 * few changed lines are. One mark per run rather than per line: the runs are what a reader
 * moves between, and a strip of a thousand hairlines is a smear.
 */
export function marksOf(rows: readonly SplitRow[]): Mark[] {
  const out: Mark[] = [];
  const total = rows.length;
  if (total === 0) return out;

  let i = 0;
  while (i < total) {
    const row = rows[i];
    if (row === undefined || !isChangedRow(row)) {
      i += 1;
      continue;
    }
    const start = i;
    let added = false;
    let removed = false;
    while (i < total) {
      const run = rows[i];
      if (run === undefined || !isChangedRow(run)) break;
      if (run.right?.kind === 'add') added = true;
      if (run.left?.kind === 'remove') removed = true;
      i += 1;
    }
    out.push({
      at: start / total,
      size: (i - start) / total,
      kind: added && removed ? 'both' : removed ? 'remove' : 'add',
    });
  }
  return out;
}
