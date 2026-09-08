/**
 * Every shape the interface draws, on one grid.
 *
 * Coral used to draw its controls with typeface glyphs — `↶` for undo, `⤓` for stash, a
 * literal `P` for patch, `>_` for the terminal. Font arrows have no bold cut, so the toolbar
 * asked for a heavier weight, got nothing, and ended up faking one with
 * `-webkit-text-stroke`. They also differ on every machine, which is the same problem the
 * bundled faces solved for text.
 *
 * These are paths instead: a 24-unit square with the drawing inside 2..22, round caps and
 * joins, and no fill unless the shape is a dot. Stroke weight is not stored here — `Icon.svelte`
 * derives it from the size it is asked for, so a glyph at eleven pixels and one at twenty land
 * on the same optical weight rather than one reading as a hairline.
 *
 * The forms follow Lucide's conventions, which the marks in this window already did; Lucide is
 * credited in CREDITS.md.
 */

/** A shape, and whether it is drawn as an outline or filled solid. */
export interface Glyph {
  /** `d` attributes stroked in order. A glyph made only of dots has none. */
  paths?: string[];
  /** Paths filled rather than stroked — a dot, a pupil, a marker. */
  solid?: string[];
}

export const ICONS = {
  /* ── history ─────────────────────────────────────────────────────────────────────── */
  undo: { paths: ['M4 11h10.5a5 5 0 0 1 0 10H10', 'M9 6 4 11l5 5'] },
  redo: { paths: ['M20 11H9.5a5 5 0 0 0 0 10H14', 'M15 6l5 5-5 5'] },
  clock: { paths: ['M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Z', 'M12 7v5.2l3.4 2'] },

  /* ── the remote ──────────────────────────────────────────────────────────────────── */
  /* Two arcs chasing each other, each with a head at its own end. */
  fetch: {
    paths: [
      'M20.5 12a8.5 8.5 0 0 1-14.9 5.6',
      'M3.5 12a8.5 8.5 0 0 1 14.9-5.6',
      'M18.5 2.5v4h-4',
      'M5.5 21.5v-4h4',
    ],
  },
  pull: { paths: ['M12 3v13', 'M6.5 10.5 12 16l5.5-5.5', 'M4 21h16'] },
  push: { paths: ['M12 21V8', 'M6.5 13.5 12 8l5.5 5.5', 'M4 3h16'] },
  cloud: { paths: ['M6.5 19a4.5 4.5 0 0 1-.6-8.96 6 6 0 0 1 11.6 1.46A4 4 0 0 1 17 19Z'] },

  /* ── the working copy ────────────────────────────────────────────────────────────── */
  /* A commit with two branches leaving it, which is the mark Coral is named after. */
  branch: {
    paths: [
      'M6 3v12',
      'M18 9a9 9 0 0 1-9 9',
      'M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z',
      'M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z',
    ],
  },
  /* One node on a lane, which is what a row of the graph is. */
  commit: { paths: ['M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z', 'M12 3v5.5', 'M12 15.5V21'] },
  /* A branch that has been offered to another: the arrow is the ask. */
  request: {
    paths: [
      'M6 9v12',
      'M6 6a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z'.replace('0 0', '0 0'),
      'M18 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z',
      'M18 15V9a3 3 0 0 0-3-3h-4',
      'M13.5 3.5 10.5 6l3 2.5',
    ],
  },
  tag: {
    paths: ['M12.9 3.2A2.5 2.5 0 0 0 11.2 2.5H5a2.5 2.5 0 0 0-2.5 2.5v6.2a2.5 2.5 0 0 0 .73 1.77l7.8 7.8a2 2 0 0 0 2.83 0l6.2-6.2a2 2 0 0 0 0-2.83Z'],
    solid: ['M8 8.5a1.4 1.4 0 1 1-2.8 0 1.4 1.4 0 0 1 2.8 0Z'],
  },
  stash: { paths: ['M3 8.5h18v11a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 19.5Z', 'M2 3.5h20v5H2Z', 'M10 13h4'] },
  /* Stash and pop are the same box; the arrow says which way the work is going. */
  stashPush: { paths: ['M3 11.5h18v8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 19.5Z', 'M12 2.5v6', 'M9 5.5l3 3 3-3'] },
  stashPop: { paths: ['M3 11.5h18v8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 19.5Z', 'M12 8.5v-6', 'M9 5.5l3-3 3 3'] },
  patch: { paths: ['M14 2.5H7A1.5 1.5 0 0 0 5.5 4v16A1.5 1.5 0 0 0 7 21.5h10a1.5 1.5 0 0 0 1.5-1.5V7Z', 'M14 2.5V7h4.5', 'M12 10.5v6', 'M9 13.5h6'] },
  terminal: { paths: ['M5 7l4.5 4.5L5 16', 'M12.5 17h6.5'] },

  /* ── a file and what happened to it ──────────────────────────────────────────────── */
  file: { paths: ['M14 2.5H7A1.5 1.5 0 0 0 5.5 4v16A1.5 1.5 0 0 0 7 21.5h10a1.5 1.5 0 0 0 1.5-1.5V7Z', 'M14 2.5V7h4.5'] },
  folder: { paths: ['M2.5 6a1.5 1.5 0 0 1 1.5-1.5h4.6a1.5 1.5 0 0 1 1.2.6l1.2 1.6h7.5A1.5 1.5 0 0 1 20 8.2v10.3a1.5 1.5 0 0 1-1.5 1.5h-14.5A1.5 1.5 0 0 1 2.5 18.5Z'] },
  diff: { paths: ['M12 3.5v7', 'M8.5 7h7', 'M8.5 17h7', 'M4 12.5h16'] },
  blame: { paths: ['M12 12.5a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z', 'M4.5 20.5a7.5 7.5 0 0 1 15 0'] },

  /* ── moving about ───────────────────────────────────────────────────────────────── */
  chevronDown: { paths: ['M6.5 9.5 12 15l5.5-5.5'] },
  chevronRight: { paths: ['M9.5 5.5 15 11l-5.5 5.5'] },
  chevronUp: { paths: ['M6.5 14.5 12 9l5.5 5.5'] },
  arrowUp: { paths: ['M12 20V4', 'M6 10l6-6 6 6'] },
  arrowDown: { paths: ['M12 4v16', 'M6 14l6 6 6-6'] },
  external: { paths: ['M13 4h7v7', 'M20 4 10.5 13.5', 'M18 14.5v4A1.5 1.5 0 0 1 16.5 20h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6h4'] },

  /* ── acting on something ─────────────────────────────────────────────────────────── */
  plus: { paths: ['M12 5v14', 'M5 12h14'] },
  minus: { paths: ['M5 12h14'] },
  close: { paths: ['M6 6l12 12', 'M18 6 6 18'] },
  check: { paths: ['M4.5 12.5 9.5 17.5 19.5 6.5'] },
  more: { solid: ['M13.4 5a1.4 1.4 0 1 1-2.8 0 1.4 1.4 0 0 1 2.8 0Z', 'M13.4 12a1.4 1.4 0 1 1-2.8 0 1.4 1.4 0 0 1 2.8 0Z', 'M13.4 19a1.4 1.4 0 1 1-2.8 0 1.4 1.4 0 0 1 2.8 0Z'] },
  search: { paths: ['M10.5 17.5a7 7 0 1 0 0-14 7 7 0 0 0 0 14Z', 'M20.5 20.5 15.6 15.6'] },
  copy: { paths: ['M9.5 9.5A1.5 1.5 0 0 1 11 8h8a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 19 19h-8a1.5 1.5 0 0 1-1.5-1.5Z', 'M15.5 8V6.5A1.5 1.5 0 0 0 14 5H5a1.5 1.5 0 0 0-1.5 1.5v9A1.5 1.5 0 0 0 5 17h1.5'] },
  edit: { paths: ['M11 5H5.5A1.5 1.5 0 0 0 4 6.5v12A1.5 1.5 0 0 0 5.5 20h12a1.5 1.5 0 0 0 1.5-1.5V13', 'M17.5 3.5a2.1 2.1 0 0 1 3 3L12.5 14.5l-4 1 1-4Z'] },
  trash: { paths: ['M4 6.5h16', 'M9.5 6.5V4.5A1 1 0 0 1 10.5 3.5h3a1 1 0 0 1 1 1v2', 'M6.5 6.5v13A1.5 1.5 0 0 0 8 21h8a1.5 1.5 0 0 0 1.5-1.5v-13', 'M10 11v5.5', 'M14 11v5.5'] },
  restart: { paths: ['M20 12a8 8 0 1 1-2.34-5.66', 'M20.5 3v5h-5'] },
  stop: { solid: ['M7.5 8.5a1 1 0 0 1 1-1h7a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1Z'] },

  /* ── the window's own chrome ─────────────────────────────────────────────────────── */
  settings: {
    paths: ['M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4Z', 'M19.1 14.4a1.6 1.6 0 0 0 .32 1.76l.06.06a1.9 1.9 0 1 1-2.7 2.7l-.05-.06a1.6 1.6 0 0 0-1.77-.32 1.6 1.6 0 0 0-.97 1.46v.17a1.9 1.9 0 1 1-3.8 0v-.09a1.6 1.6 0 0 0-1.04-1.46 1.6 1.6 0 0 0-1.76.32l-.06.06a1.9 1.9 0 1 1-2.7-2.7l.06-.06a1.6 1.6 0 0 0 .32-1.76 1.6 1.6 0 0 0-1.46-.97H3.3a1.9 1.9 0 1 1 0-3.8h.09a1.6 1.6 0 0 0 1.46-1.04 1.6 1.6 0 0 0-.32-1.76l-.06-.06a1.9 1.9 0 1 1 2.7-2.7l.06.06a1.6 1.6 0 0 0 1.76.32h.08A1.6 1.6 0 0 0 10 3.96V3.8a1.9 1.9 0 1 1 3.8 0v.09a1.6 1.6 0 0 0 .97 1.46 1.6 1.6 0 0 0 1.76-.32l.06-.06a1.9 1.9 0 1 1 2.7 2.7l-.06.06a1.6 1.6 0 0 0-.32 1.76v.08a1.6 1.6 0 0 0 1.46.97h.17a1.9 1.9 0 1 1 0 3.8h-.09a1.6 1.6 0 0 0-1.46.97Z'],
  },
  sun: { paths: ['M12 16.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9Z', 'M12 2.5v2M12 19.5v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2.5 12h2M19.5 12h2M6.3 17.7l-1.4 1.4M19.1 4.9l-1.4 1.4'] },
  moon: { paths: ['M20.5 14.3A8.5 8.5 0 0 1 9.7 3.5a8.5 8.5 0 1 0 10.8 10.8Z'] },
  monitor: { paths: ['M3.5 5.5A1.5 1.5 0 0 1 5 4h14a1.5 1.5 0 0 1 1.5 1.5v9A1.5 1.5 0 0 1 19 16H5a1.5 1.5 0 0 1-1.5-1.5Z', 'M8.5 20h7', 'M12 16v4'] },
  logs: { paths: ['M4 5.5h.01M4 12h.01M4 18.5h.01', 'M8.5 5.5H20M8.5 12H20M8.5 18.5H20'] },
  minimise: { paths: ['M7 12h10'] },
  maximise: { paths: ['M6 6.5A.5.5 0 0 1 6.5 6h11a.5.5 0 0 1 .5.5v11a.5.5 0 0 1-.5.5h-11a.5.5 0 0 1-.5-.5Z'] },
  restore: { paths: ['M9 6.5A1.5 1.5 0 0 1 10.5 5h7A1.5 1.5 0 0 1 19 6.5v7a1.5 1.5 0 0 1-1.5 1.5', 'M5 10.5A1.5 1.5 0 0 1 6.5 9h7a1.5 1.5 0 0 1 1.5 1.5v7a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 5 17.5Z'] },

  /* ── what is showing, and what is not ────────────────────────────────────────────── */
  eye: { paths: ['M2.5 12S6.1 5.5 12 5.5 21.5 12 21.5 12 17.9 18.5 12 18.5 2.5 12 2.5 12Z', 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z'] },
  eyeOff: { paths: ['M10.7 5.7A9.6 9.6 0 0 1 12 5.5c5.9 0 9.5 6.5 9.5 6.5a17 17 0 0 1-2.3 3.2', 'M6.7 7.1A17 17 0 0 0 2.5 12s3.6 6.5 9.5 6.5a9.4 9.4 0 0 0 4.1-.9', 'M14.1 14.1a3 3 0 1 1-4.2-4.2', 'M3.5 3.5l17 17'] },
  filter: { paths: ['M3.5 5.5h17l-6.5 7.7v6.3l-4 1.5v-7.8Z'] },
  /* Keys and signatures: what the repository is guarded with. */
  shield: { paths: ['M12 21.5s7.5-3.5 7.5-9.5V5.6l-7.5-3-7.5 3v6.4c0 6 7.5 9.5 7.5 9.5Z'] },
  /* Settings that are still being tried out. */
  beaker: { paths: ['M9.5 2.5h5', 'M10 2.5v6.2L4.7 18a2 2 0 0 0 1.7 3h11.2a2 2 0 0 0 1.7-3L14 8.7V2.5', 'M7.2 14h9.6'] },

  /* ── saying how something went ───────────────────────────────────────────────────── */
  alert: { paths: ['M12 9v4.5', 'M10.6 3.9a1.6 1.6 0 0 1 2.8 0l7.4 13.1a1.6 1.6 0 0 1-1.4 2.4H4.6a1.6 1.6 0 0 1-1.4-2.4Z'], solid: ['M13.1 16.9a1.1 1.1 0 1 1-2.2 0 1.1 1.1 0 0 1 2.2 0Z'] },
  info: { paths: ['M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z', 'M12 11.5v5'], solid: ['M13.1 7.8a1.1 1.1 0 1 1-2.2 0 1.1 1.1 0 0 1 2.2 0Z'] },
  tick: { paths: ['M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z', 'M8 12.2 10.9 15 16 9.5'] },
  cross: { paths: ['M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z', 'M9 9l6 6M15 9l-6 6'] },

  /* ── the diff's own controls ─────────────────────────────────────────────────────── */
  columns: { paths: ['M3.5 5.5A1.5 1.5 0 0 1 5 4h14a1.5 1.5 0 0 1 1.5 1.5v13A1.5 1.5 0 0 1 19 20H5a1.5 1.5 0 0 1-1.5-1.5Z', 'M12 4v16'] },
  rows: { paths: ['M3.5 5.5A1.5 1.5 0 0 1 5 4h14a1.5 1.5 0 0 1 1.5 1.5v13A1.5 1.5 0 0 1 19 20H5a1.5 1.5 0 0 1-1.5-1.5Z', 'M3.5 12h17'] },
  pilcrow: { paths: ['M12 4v16', 'M17 4v16', 'M17 4h-6.5a3.5 3.5 0 0 0 0 7H12'] },
  unfold: { paths: ['M8 5l4-2.5L16 5', 'M16 19l-4 2.5L8 19', 'M4 12h16'] },
} satisfies Record<string, Glyph>;

/** The name of a shape in the set. */
export type IconName = keyof typeof ICONS;

/**
 * How thick to stroke a glyph so it lands on the same weight at every size.
 *
 * The paths are drawn on a 24-unit grid, so a fixed stroke width shrinks with the icon: two
 * units is 1.3px at sixteen but 0.9px at eleven, which WebKit renders as a smudge. This
 * targets a constant 1.35px on screen and stops short at each end, since a very small glyph
 * given its full share of stroke closes up into a blob.
 */
export function strokeFor(size: number): number {
  return Math.min(2.8, Math.max(1.6, (1.35 * 24) / size));
}
