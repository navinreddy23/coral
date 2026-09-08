/**
 * Every shape the interface draws, on one grid, in one vocabulary.
 *
 * The vocabulary is the graph's own: **a lane is stroked and a commit is filled**. That is
 * what `GraphCanvas` draws a million times over, and it is what the mark in the corner is made
 * of, so it is what the icons are made of too. Generalised to the rest of the set the rule
 * reads: stroke the structure, fill the subject. A gear is stroked and its hub is solid; an
 * eye is stroked and its pupil is solid; a page is stroked and the line added to it is solid.
 *
 * The rule is not decoration. The first version of this set was drawn entirely in hairlines of
 * one weight, and at sixteen pixels a row of it read as grey noise — nothing in any glyph was
 * heavy enough to land on first. Fill is what gives a glyph somewhere for the eye to go.
 *
 * The four the toolbar leans on hardest are drawn from git rather than from file transfer.
 * Pull and push were a download arrow and an upload arrow, which is what every application
 * that moves bytes uses; here the bar is where a commit comes from, the solid disc is where it
 * ends up, and the two mirror each other. Stash and pop put that disc onto a shelf and take it
 * back off. Patch is a page with one line added and one removed, rather than a page with a plus
 * on it, which is what "new file" looks like everywhere there is.
 *
 * A 24-unit square with the drawing inside 2..22, round caps and joins. Stroke weight is not
 * stored here — `Icon.svelte` derives it from the size it is asked for, so a glyph at eleven
 * pixels and one at twenty-four land on the same optical weight.
 */

/** A shape: what is stroked, what is filled, and where its solid discs are. */
export interface Glyph {
  /** `d` attributes stroked in order — the structure. */
  paths?: string[];
  /** `d` attributes filled — the subject. */
  solid?: string[];
  /** Filled discs, as `[x, y, radius]`. A commit, a hub, a pupil, a bullet. */
  dots?: [number, number, number][];
}

export const ICONS = {
  /* ── history ─────────────────────────────────────────────────────────────────────── */
  undo: { paths: ['M6.5 11.5h8.5a5 5 0 0 1 0 10h-4.5'], solid: ['M8.6 5.6 2.6 11.5l6 5.9Z'] },
  redo: { paths: ['M17.5 11.5H9a5 5 0 0 0 0 10h4.5'], solid: ['M15.4 5.6l6 5.9-6 5.9Z'] },
  clock: {
    paths: ['M12 3.2a8.8 8.8 0 1 0 0 17.6 8.8 8.8 0 0 0 0-17.6Z', 'M12 7.4V12l3.2 1.9'],
    dots: [[12, 12, 1.5]],
  },

  /* ── the remote ──────────────────────────────────────────────────────────────────── */
  /*
   * Fetch goes and looks; pull brings something back and lands it. So fetch circles a commit
   * and pull arrives on one — which is also what tells the two apart in a toolbar, where two
   * arrows a centimetre apart would not.
   */
  fetch: {
    paths: ['M19.5 12a7.5 7.5 0 1 1-2.2-5.3'],
    solid: ['M20.2 3.4 20.9 9.6l-5.9-2Z'],
    dots: [[12, 12, 2.6]],
  },
  /* The bar is where the commit comes from and the disc is where it ends up. Push is the same
     drawing turned over, which is the whole of the difference between them. */
  pull: {
    paths: ['M4 3.5h16', 'M12 6.5v4.5'],
    solid: ['M12 15.4 8 10.2h8Z'],
    dots: [[12, 19.6, 2.6]],
  },
  push: {
    paths: ['M4 20.5h16', 'M12 17.5V13'],
    solid: ['M12 8.6 16 13.8H8Z'],
    dots: [[12, 4.4, 2.6]],
  },
  cloud: { solid: ['M6.6 19.4a4.6 4.6 0 0 1-.7-9.15 6.1 6.1 0 0 1 11.8 1.5A4.1 4.1 0 0 1 17.2 19.4Z'] },

  /* ── the working copy ────────────────────────────────────────────────────────────── */
  /* One lane, two commits, and a second lane peeling off to a third. */
  branch: {
    paths: ['M6 7.4v9.2', 'M6 12h6a4 4 0 0 0 4-4V7.4'],
    dots: [[6, 4.6, 2.7], [6, 19.4, 2.7], [16, 4.6, 2.7]],
  },
  /* One node on a lane, which is what a row of the graph is. */
  commit: { paths: ['M12 3v5.4', 'M12 15.6V21'], dots: [[12, 12, 3.6]] },
  /* A branch offered to another: two lanes, and an arrow asking. */
  request: {
    paths: ['M5.5 8.6v9.4', 'M18.5 8.6v2.8a3.6 3.6 0 0 1-3.6 3.6h-2.6'],
    solid: ['M13.4 11 9.4 15l4 4Z'],
    dots: [[5.5, 5.6, 2.7], [18.5, 5.6, 2.7], [5.5, 20.8, 2.7]],
  },
  tag: {
    paths: ['M12.9 3.2A2.5 2.5 0 0 0 11.2 2.5H5a2.5 2.5 0 0 0-2.5 2.5v6.2a2.5 2.5 0 0 0 .73 1.77l7.8 7.8a2 2 0 0 0 2.83 0l6.2-6.2a2 2 0 0 0 0-2.83Z'],
    dots: [[7.2, 7.4, 1.7]],
  },
  /* The box a stash sits in, for the sidebar's heading and a stash's own label. */
  stash: {
    paths: ['M4 9.4h16v9.6a1.6 1.6 0 0 1-1.6 1.6H5.6A1.6 1.6 0 0 1 4 19Z', 'M10 14h4'],
    solid: ['M3.4 3.4h17.2a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1H3.4a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1Z'],
  },
  /*
   * Into the box, and back out of it — the same box the sidebar puts over its stash list.
   *
   * A shelf was tried first and had to go: a shelf with an arrow over it has the same
   * silhouette as pull and push do, and the two pairs sit in adjacent groups on the toolbar.
   * A box does not look like a bar.
   */
  stashPush: {
    paths: ['M12 2.6v2.6', 'M5.2 15.6h13.6v3.8a1.6 1.6 0 0 1-1.6 1.6H6.8a1.6 1.6 0 0 1-1.6-1.6Z'],
    solid: [
      'M12 11.4 7.8 5.2h8.4Z',
      'M3.6 12.4h16.8a1 1 0 0 1 1 1v1.2a1 1 0 0 1-1 1H3.6a1 1 0 0 1-1-1v-1.2a1 1 0 0 1 1-1Z',
    ],
  },
  stashPop: {
    paths: ['M12 11.4V8.8', 'M5.2 15.6h13.6v3.8a1.6 1.6 0 0 1-1.6 1.6H6.8a1.6 1.6 0 0 1-1.6-1.6Z'],
    solid: [
      'M12 2.6 16.2 8.8H7.8Z',
      'M3.6 12.4h16.8a1 1 0 0 1 1 1v1.2a1 1 0 0 1-1 1H3.6a1 1 0 0 1-1-1v-1.2a1 1 0 0 1 1-1Z',
    ],
  },
  /* A page with one line added and one taken away. Not a page with a plus on it, which is what
     "new file" looks like in every application there is. */
  patch: {
    paths: [
      'M13.6 2.5H7A1.5 1.5 0 0 0 5.5 4v16A1.5 1.5 0 0 0 7 21.5h10a1.5 1.5 0 0 0 1.5-1.5V7.4Z',
      'M13.6 2.5v4.9h4.9',
      'M8.8 16.6h6.4',
    ],
    solid: ['M8.8 11.2h6.4v2.4H8.8Z'],
  },
  terminal: { paths: ['M4.5 6.5 10 12l-5.5 5.5'], solid: ['M12.5 15.6h7v2.4h-7Z'] },

  /* ── a file and what happened to it ──────────────────────────────────────────────── */
  file: {
    paths: ['M13.6 2.5H7A1.5 1.5 0 0 0 5.5 4v16A1.5 1.5 0 0 0 7 21.5h10a1.5 1.5 0 0 0 1.5-1.5V7.4Z'],
    solid: ['M13.6 2.5 18.5 7.4h-4.9Z'],
  },
  folder: {
    paths: ['M2.5 7.6h19a1 1 0 0 1 1 1v9.9a1.5 1.5 0 0 1-1.5 1.5h-17A1.5 1.5 0 0 1 2.5 18.5Z'],
    solid: ['M2.5 6.6V5.8a1.5 1.5 0 0 1 1.5-1.5h4.4a1.5 1.5 0 0 1 1.2.6l1.1 1.7Z'],
  },
  /* One line added and one removed, which is what a diff is. */
  diff: { paths: ['M4 16.2h16'], solid: ['M4 6.2h16v3.2H4Z'] },
  blame: { paths: ['M4.6 20.5a7.4 7.4 0 0 1 14.8 0'], dots: [[12, 8, 4]] },

  /* ── moving about ───────────────────────────────────────────────────────────────── */
  chevronDown: { paths: ['M6 9.5 12 15.5l6-6'] },
  chevronRight: { paths: ['M9.5 6 15.5 12l-6 6'] },
  chevronUp: { paths: ['M6 14.5 12 8.5l6 6'] },
  arrowUp: { paths: ['M12 20.5V6'], solid: ['M12 2.4 18 8.6H6Z'] },
  arrowDown: { paths: ['M12 3.5V18'], solid: ['M12 21.6 6 15.4h12Z'] },
  external: {
    paths: [
      'M18 13.6v5A1.4 1.4 0 0 1 16.6 20H5.4A1.4 1.4 0 0 1 4 18.6V7.4A1.4 1.4 0 0 1 5.4 6h5',
      'M20 4 12.4 11.6',
    ],
    solid: ['M13.8 3.2h7v7l-2.6-2.1V5.8h-2.1Z'],
  },

  /* ── acting on something ─────────────────────────────────────────────────────────── */
  plus: { paths: ['M12 5v14', 'M5 12h14'] },
  minus: { paths: ['M5 12h14'] },
  close: { paths: ['M6 6l12 12', 'M18 6 6 18'] },
  check: { paths: ['M4.5 12.5 9.5 17.5 19.5 6.5'] },
  more: { dots: [[12, 5, 1.6], [12, 12, 1.6], [12, 19, 1.6]] },
  search: {
    paths: ['M10.6 17.2a6.6 6.6 0 1 0 0-13.2 6.6 6.6 0 0 0 0 13.2Z', 'M20.4 20.4 15.4 15.4'],
  },
  copy: {
    paths: ['M9.5 9.5A1.5 1.5 0 0 1 11 8h8a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 19 19h-8a1.5 1.5 0 0 1-1.5-1.5Z'],
    solid: ['M5 3.6h9a2.5 2.5 0 0 1 2.5 2.5v.5H11a3 3 0 0 0-3 3v6.6H5A2.5 2.5 0 0 1 2.5 13.7V6.1A2.5 2.5 0 0 1 5 3.6Z'],
  },
  edit: {
    paths: ['M11 5H5.5A1.5 1.5 0 0 0 4 6.5v12A1.5 1.5 0 0 0 5.5 20h12a1.5 1.5 0 0 0 1.5-1.5V13'],
    solid: ['M17.6 3.4a2.1 2.1 0 0 1 3 3l-1 1-3-3Z', 'M15.5 5.5l3 3-6 6-4 1 1-4Z'],
  },
  trash: {
    paths: [
      'M6.6 8.4v11A1.6 1.6 0 0 0 8.2 21h7.6a1.6 1.6 0 0 0 1.6-1.6v-11',
      'M10 12v5',
      'M14 12v5',
    ],
    solid: ['M3.6 5.2h16.8v2.4H3.6Z', 'M9.4 2.6h5.2a1 1 0 0 1 1 1v.9H8.4v-.9a1 1 0 0 1 1-1Z'],
  },
  restart: { paths: ['M20 12a8 8 0 1 1-3.4-6.5'], solid: ['M21 3.2v6.2h-6Z'] },
  stop: { solid: ['M7.6 8.6a1 1 0 0 1 1-1h6.8a1 1 0 0 1 1 1v6.8a1 1 0 0 1-1 1H8.6a1 1 0 0 1-1-1Z'] },

  /* ── the window's own chrome ─────────────────────────────────────────────────────── */
  settings: {
    paths: ['M19.1 14.4a1.6 1.6 0 0 0 .32 1.76l.06.06a1.9 1.9 0 1 1-2.7 2.7l-.05-.06a1.6 1.6 0 0 0-1.77-.32 1.6 1.6 0 0 0-.97 1.46v.17a1.9 1.9 0 1 1-3.8 0v-.09a1.6 1.6 0 0 0-1.04-1.46 1.6 1.6 0 0 0-1.76.32l-.06.06a1.9 1.9 0 1 1-2.7-2.7l.06-.06a1.6 1.6 0 0 0 .32-1.76 1.6 1.6 0 0 0-1.46-.97H3.3a1.9 1.9 0 1 1 0-3.8h.09a1.6 1.6 0 0 0 1.46-1.04 1.6 1.6 0 0 0-.32-1.76l-.06-.06a1.9 1.9 0 1 1 2.7-2.7l.06.06a1.6 1.6 0 0 0 1.76.32h.08A1.6 1.6 0 0 0 10 3.96V3.8a1.9 1.9 0 1 1 3.8 0v.09a1.6 1.6 0 0 0 .97 1.46 1.6 1.6 0 0 0 1.76-.32l.06-.06a1.9 1.9 0 1 1 2.7 2.7l-.06.06a1.6 1.6 0 0 0-.32 1.76v.08a1.6 1.6 0 0 0 1.46.97h.17a1.9 1.9 0 1 1 0 3.8h-.09a1.6 1.6 0 0 0-1.46.97Z'],
    dots: [[12, 12, 2.9]],
  },
  sun: {
    paths: ['M12 2.4v2.2M12 19.4v2.2M4.8 4.8l1.6 1.6M17.6 17.6l1.6 1.6M2.4 12h2.2M19.4 12h2.2M6.4 17.6l-1.6 1.6M19.2 4.8l-1.6 1.6'],
    dots: [[12, 12, 4.4]],
  },
  moon: { solid: ['M20.6 14.4A8.6 8.6 0 0 1 9.6 3.4a8.6 8.6 0 1 0 11 11Z'] },
  monitor: {
    paths: ['M3.5 5.6A1.6 1.6 0 0 1 5.1 4h13.8a1.6 1.6 0 0 1 1.6 1.6v8.8a1.6 1.6 0 0 1-1.6 1.6H5.1a1.6 1.6 0 0 1-1.6-1.6Z'],
    solid: ['M10.8 16h2.4v3.4h2.9v2.1H7.9v-2.1h2.9Z'],
  },
  logs: {
    paths: ['M8.6 5.4H20M8.6 12H20M8.6 18.6H20'],
    dots: [[4.4, 5.4, 1.5], [4.4, 12, 1.5], [4.4, 18.6, 1.5]],
  },
  minimise: { paths: ['M7 12h10'] },
  maximise: { paths: ['M6 6.5A.5.5 0 0 1 6.5 6h11a.5.5 0 0 1 .5.5v11a.5.5 0 0 1-.5.5h-11a.5.5 0 0 1-.5-.5Z'] },
  restore: {
    paths: [
      'M9 6.5A1.5 1.5 0 0 1 10.5 5h7A1.5 1.5 0 0 1 19 6.5v7a1.5 1.5 0 0 1-1.5 1.5',
      'M5 10.5A1.5 1.5 0 0 1 6.5 9h7a1.5 1.5 0 0 1 1.5 1.5v7a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 5 17.5Z',
    ],
  },

  /* ── what is showing, and what is not ────────────────────────────────────────────── */
  eye: {
    paths: ['M2.5 12S6.1 5.5 12 5.5 21.5 12 21.5 12 17.9 18.5 12 18.5 2.5 12 2.5 12Z'],
    dots: [[12, 12, 2.9]],
  },
  eyeOff: {
    paths: [
      'M10.7 5.7A9.6 9.6 0 0 1 12 5.5c5.9 0 9.5 6.5 9.5 6.5a17 17 0 0 1-2.3 3.2',
      'M6.7 7.1A17 17 0 0 0 2.5 12s3.6 6.5 9.5 6.5a9.4 9.4 0 0 0 4.1-.9',
      'M14.1 14.1a3 3 0 1 1-4.2-4.2',
      'M3.5 3.5l17 17',
    ],
  },
  filter: { solid: ['M3.4 4.6h17.2l-6.9 8.2v6.5l-3.4 1.7v-8.2Z'] },

  /* ── saying how something went ───────────────────────────────────────────────────── */
  alert: {
    paths: ['M10.6 3.9a1.6 1.6 0 0 1 2.8 0l7.4 13.1a1.6 1.6 0 0 1-1.4 2.4H4.6a1.6 1.6 0 0 1-1.4-2.4Z'],
    solid: ['M10.9 8.6h2.2v5.6h-2.2Z'],
    dots: [[12, 16.8, 1.2]],
  },
  info: {
    paths: ['M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z'],
    solid: ['M10.9 10.8h2.2v6h-2.2Z'],
    dots: [[12, 7.6, 1.2]],
  },
  tick: { paths: ['M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z', 'M8 12.2 10.9 15 16 9.5'] },
  cross: { paths: ['M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z', 'M9 9l6 6M15 9l-6 6'] },
  /* Keys and signatures: what the repository is guarded with. */
  shield: { solid: ['M12 21.7 11.5 21.4C9 20 4.9 16.7 4.9 11.9V5.5L12 2.6l7.1 2.9v6.4c0 4.8-4.1 8.1-6.6 9.5Z'] },
  /* Settings that are still being tried out. */
  beaker: {
    paths: ['M9.5 2.6h5', 'M10 2.6v6.2L4.7 18a2 2 0 0 0 1.7 3h11.2a2 2 0 0 0 1.7-3L14 8.8V2.6'],
    solid: ['M7.1 14.6h9.8l2.4 3.9a1.4 1.4 0 0 1-1.2 2.1H5.9a1.4 1.4 0 0 1-1.2-2.1Z'],
  },

  /* ── the diff's own controls ─────────────────────────────────────────────────────── */
  /*
   * The two side panels, and what they look like folded away.
   *
   * The glyph carries the state rather than the button under it: a filled strip is a panel
   * that is showing and a ruled edge is one that is not. Lighting the button instead would
   * have both of them lit whenever the window is in the state it ships in, which is a toolbar
   * that glows for no reason.
   */
  panelLeft: {
    paths: ['M3.5 5.6A1.6 1.6 0 0 1 5.1 4h13.8a1.6 1.6 0 0 1 1.6 1.6v12.8a1.6 1.6 0 0 1-1.6 1.6H5.1a1.6 1.6 0 0 1-1.6-1.6Z'],
    solid: ['M5.1 5.6h4.3v12.8H5.1Z'],
  },
  panelLeftOff: {
    paths: [
      'M3.5 5.6A1.6 1.6 0 0 1 5.1 4h13.8a1.6 1.6 0 0 1 1.6 1.6v12.8a1.6 1.6 0 0 1-1.6 1.6H5.1a1.6 1.6 0 0 1-1.6-1.6Z',
      'M9.4 5.6v12.8',
    ],
  },
  panelRight: {
    paths: ['M3.5 5.6A1.6 1.6 0 0 1 5.1 4h13.8a1.6 1.6 0 0 1 1.6 1.6v12.8a1.6 1.6 0 0 1-1.6 1.6H5.1a1.6 1.6 0 0 1-1.6-1.6Z'],
    solid: ['M14.6 5.6h4.3v12.8h-4.3Z'],
  },
  panelRightOff: {
    paths: [
      'M3.5 5.6A1.6 1.6 0 0 1 5.1 4h13.8a1.6 1.6 0 0 1 1.6 1.6v12.8a1.6 1.6 0 0 1-1.6 1.6H5.1a1.6 1.6 0 0 1-1.6-1.6Z',
      'M14.6 5.6v12.8',
    ],
  },
  columns: {
    paths: ['M3.5 5.6A1.6 1.6 0 0 1 5.1 4h13.8a1.6 1.6 0 0 1 1.6 1.6v12.8a1.6 1.6 0 0 1-1.6 1.6H5.1a1.6 1.6 0 0 1-1.6-1.6Z'],
    solid: ['M5.1 5.6h5.7v12.8H5.1Z'],
  },
  rows: {
    paths: ['M3.5 5.6A1.6 1.6 0 0 1 5.1 4h13.8a1.6 1.6 0 0 1 1.6 1.6v12.8a1.6 1.6 0 0 1-1.6 1.6H5.1a1.6 1.6 0 0 1-1.6-1.6Z'],
    solid: ['M3.5 5.6h17v5.4h-17Z'],
  },
  pilcrow: { paths: ['M17 4v16', 'M12 4v16'], solid: ['M12.6 3.4h1.6v8.8h-1.6a4.4 4.4 0 0 1 0-8.8Z'] },
  unfold: { paths: ['M4 12h16'], solid: ['M12 2.4 16.4 7.4H7.6Z', 'M12 21.6 7.6 16.6h8.8Z'] },
} satisfies Record<string, Glyph>;

/** The name of a shape in the set. */
export type IconName = keyof typeof ICONS;

/**
 * How thick to stroke a glyph so it lands on the same weight at every size.
 *
 * The paths are drawn on a 24-unit grid, so a fixed stroke width shrinks with the icon: two
 * units is 1.3px at sixteen but 0.9px at eleven, which WebKit renders as a smudge. This targets
 * a constant 1.45px on screen — a little heavier than the hairline the first set used, so the
 * strokes hold their own beside the filled masses — and stops short at each end, since a very
 * small glyph given its full share of stroke closes up into a blob.
 */
export function strokeFor(size: number): number {
  return Math.min(3, Math.max(1.7, (1.45 * 24) / size));
}
