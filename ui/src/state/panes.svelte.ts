import { GRAPH_COLUMN_PX, REFS_COLUMN_PX } from '../graph/layout';

/** Every width the user can drag, in CSS pixels. */
export interface PaneWidths {
  sidebar: number;
  details: number;
  refs: number;
  graph: number;
}

export type PaneKey = keyof PaneWidths;

/**
 * How far each width may be dragged.
 *
 * A pane that can be dragged to nothing is a pane the user cannot get back, since the handle
 * goes with it; the floors here keep every handle reachable.
 */
export const PANE_LIMITS: Record<PaneKey, { min: number; max: number }> = {
  sidebar: { min: 150, max: 520 },
  details: { min: 220, max: 720 },
  refs: { min: 60, max: 480 },
  graph: { min: 60, max: 640 },
};

const STORAGE_KEY = 'coral.panes';

export function clampPane(key: PaneKey, px: number): number {
  const { min, max } = PANE_LIMITS[key];
  return Math.min(max, Math.max(min, Math.round(px)));
}

/** Widths of the resizable panes and columns, remembered across launches. */
export class PanesState {
  widths = $state<PaneWidths>(defaults());

  constructor() {
    const stored = read();
    if (stored) this.widths = stored;
  }

  resize(key: PaneKey, px: number): void {
    this.widths = { ...this.widths, [key]: clampPane(key, px) };
    write(this.widths);
  }

  /** Back to the shipped layout, for when a drag has left something unusable. */
  reset(): void {
    this.widths = defaults();
    write(this.widths);
  }
}

function defaults(): PaneWidths {
  return { sidebar: 240, details: 340, refs: REFS_COLUMN_PX, graph: GRAPH_COLUMN_PX };
}

/**
 * Reads the stored widths, clamping each one.
 *
 * The limits can change between releases, and a width stored under the old ones would
 * otherwise come back as a pane that cannot be dragged back into view.
 */
function read(): PaneWidths | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return null;

    const out = defaults();
    for (const key of Object.keys(out) as PaneKey[]) {
      const value = (parsed as Record<string, unknown>)[key];
      if (typeof value === 'number' && Number.isFinite(value)) out[key] = clampPane(key, value);
    }
    return out;
  } catch {
    // A webview with storage disabled, or a corrupt entry, must still open the window.
    return null;
  }
}

function write(widths: PaneWidths): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(widths));
  } catch {
    // Losing the layout is not worth failing over.
  }
}
