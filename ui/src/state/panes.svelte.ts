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
  // Enough for the widest graph the engine will draw: `graph::lanes::MAX_LANES` at the lane
  // pitch, plus the node's own radius and a gap before the message. A cap below that leaves
  // commits drawn off the side of the column, which reads as a graph with no commits in it.
  graph: { min: 60, max: 720 },
};

/**
 * How much of the commit list the message column is aimed at keeping.
 *
 * Not a guarantee: the two columns beside it have floors of their own, and below a certain
 * pane width those floors win. A target, not a contract.
 */
export const MESSAGE_FLOOR_PX = 260;

/**
 * The refs and graph columns as they should actually be drawn in a pane of `paneWidth`.
 *
 * Both are dragged widths that knew nothing about the pane holding them. At the window's own
 * minimum width with both side panels open they came to more than the pane, so the message
 * column — the one the list exists for — was given nothing at all, and the commit list showed
 * no messages, no dates and no object ids. The graph column already capped itself at a share
 * of the pane for this reason; this is the same rule applied to the pair.
 *
 * The stored widths are untouched, so widening the window puts them back as they were.
 */
export function fitColumns(
  widths: PaneWidths,
  paneWidth: number,
): { refs: number; graph: number } {
  const both = widths.refs + widths.graph;
  const room = paneWidth - MESSAGE_FLOOR_PX;
  if (paneWidth <= 0 || both <= room) return { refs: widths.refs, graph: widths.graph };
  const scale = Math.max(0, room) / both;
  return {
    refs: Math.max(PANE_LIMITS.refs.min, Math.round(widths.refs * scale)),
    graph: Math.max(PANE_LIMITS.graph.min, Math.round(widths.graph * scale)),
  };
}

const STORAGE_KEY = 'coral.panes';

export function clampPane(key: PaneKey, px: number): number {
  const { min, max } = PANE_LIMITS[key];
  return Math.min(max, Math.max(min, Math.round(px)));
}

/** Widths of the resizable panes and columns, remembered across launches. */
export class PanesState {
  widths = $state<PaneWidths>(defaults());

  /**
   * Whether the graph column still follows the lanes on screen.
   *
   * One shipped width is wrong in both directions: it is a corridor of nothing on a linear
   * repository, and too narrow for a merge-heavy one. So the column fits itself until the
   * handle is dragged, and a drag is taken as the width the user wants kept.
   */
  graphAuto = $state(true);

  constructor() {
    const stored = read();
    if (stored) {
      this.widths = stored.widths;
      this.graphAuto = stored.graphAuto;
    }
  }

  resize(key: PaneKey, px: number): void {
    this.widths = { ...this.widths, [key]: clampPane(key, px) };
    if (key === 'graph') this.graphAuto = false;
    this.save();
  }

  /**
   * Widens the graph column to hold `px` of lanes.
   *
   * Only ever wider. A column that shrank again would shift every commit message sideways
   * each time a merge cluster scrolled off the screen, and a little unused width is cheaper
   * than a list that moves under the eye.
   *
   * It widens whether or not the width was last set by hand. Dragging the column used to stop
   * the fit for good, so scrolling into a merge-heavy stretch of the kernel left every node
   * drawn off the side of a column nothing would widen again — a graph with no commits in it.
   * What a drag settles is how narrow the column may be, not how wide.
   */
  fitGraph(px: number): void {
    const want = clampPane('graph', px);
    if (want > this.widths.graph) this.widths = { ...this.widths, graph: want };
  }

  /** Starts the fit again, for a repository whose graph is a different shape. */
  refit(): void {
    if (!this.graphAuto) return;
    this.widths = { ...this.widths, graph: PANE_LIMITS.graph.min };
  }

  /** Back to the shipped layout, for when a drag has left something unusable. */
  reset(): void {
    this.widths = defaults();
    this.graphAuto = true;
    this.save();
  }

  // The fitted width is not written: it belongs to the repository on screen, not to the user.
  private save(): void {
    write(this.widths, this.graphAuto);
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
function read(): { widths: PaneWidths; graphAuto: boolean } | null {
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
    // Absent for anyone whose layout was stored before the column could fit itself, and
    // following the lanes is the better of the two answers for them.
    const pinned = (parsed as Record<string, unknown>)['graphAuto'];
    return { widths: out, graphAuto: pinned !== false };
  } catch {
    // A webview with storage disabled, or a corrupt entry, must still open the window.
    return null;
  }
}

function write(widths: PaneWidths, graphAuto: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...widths, graphAuto }));
  } catch {
    // Losing the layout is not worth failing over.
  }
}
