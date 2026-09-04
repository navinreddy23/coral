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

/** Where the first change is, or -1 when the two sides are identical throughout. */
export function firstChangedRow(rows: readonly SplitRow[]): number {
  return rows.findIndex(
    (row) => row.left?.kind !== 'context' || row.right?.kind !== 'context',
  );
}
