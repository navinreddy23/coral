import type { Density } from '../graph/layout';

/**
 * The choices a user makes about how to look at things, remembered across launches.
 *
 * Every one of these is a preference, not a mode: nobody picks side-by-side once and means it
 * for one file. Making the window forget them on every restart is what turns a setting into a
 * chore, so they live in one place rather than as a `$state` inside each component.
 *
 * Stored in `localStorage` rather than in the session file, because they belong to the person
 * at the window and not to the repositories that happen to be open in it.
 */

/** How a list of files is arranged. */
export type Grouping = 'path' | 'tree';

/** How a diff is laid out. */
export type DiffMode = 'inline' | 'split';

/** What the file panel is showing: the change, who wrote each line, or what touched it. */
export type FileView = 'diff' | 'blame' | 'history';

/** Where the terminal sits. */
export type Dock = 'bottom' | 'right';

/**
 * How much of a side panel is showing.
 *
 * `rail` is the sidebar minimised to its section icons: on a laptop the two panels take three
 * hundred pixels between them, which is most of a diff, and hiding the left one entirely puts
 * the branch list behind a keystroke. The right panel has no rail — it holds one thing, the
 * commit under the cursor, so there is nothing there to minimise it to.
 */
export type PanelState = 'open' | 'rail' | 'hidden';

export type { Density } from '../graph/layout';

export interface Views {
  /**
   * How much air a row is given, in the graph and in every list that follows it.
   *
   * Dense by default: this window is read on repositories with a million commits, and four
   * more of them on screen is worth more than four pixels of air around each.
   */
  density: Density;
  /** The commit detail panel's file list. */
  commitFiles: Grouping;
  /** The staging panel's two lists. */
  changes: Grouping;
  diff: DiffMode;
  fileView: FileView;
  /** Whether a change that is only whitespace counts as a change. */
  ignoreWhitespace: boolean;
  /** How much of the left panel is showing, and whether the other two are. */
  sidebar: PanelState;
  details: boolean;
  toolbar: boolean;
  terminalDock: Dock;
  terminalSize: number;
  /** The shell the terminal runs. Empty means whatever this machine would use. */
  terminalShell: string;
  /**
   * Whether that shell reads the login files.
   *
   * `null` leaves it to the platform, which is the only sensible default: macOS gets its
   * `PATH` from `/etc/zprofile` and cannot skip them, and a Linux desktop has already read
   * them for the session Coral was started from.
   */
  terminalLogin: boolean | null;
  /**
   * Whether the desktop draws the title bar instead of Coral.
   *
   * Off by default, which is what buys back the row: Coral's tab strip is the title bar. It is
   * here at all because a window manager that handles an undecorated window badly leaves no
   * way back from inside the window, and this is that way back.
   */
  systemTitleBar: boolean;
  /** Which sections of the sidebar are closed, by key. */
  collapsed: Record<string, boolean>;
}

const KEY = 'coral.views';

function defaults(): Views {
  return {
    density: 'default',
    commitFiles: 'path',
    changes: 'tree',
    diff: 'inline',
    fileView: 'diff',
    ignoreWhitespace: false,
    sidebar: 'open',
    details: true,
    toolbar: true,
    terminalDock: 'bottom',
    terminalSize: 260,
    terminalShell: '',
    terminalLogin: null,
    systemTitleBar: false,
    // Remote and tag lists run to hundreds on a real repository, so they start closed; local
    // branches are what people look at.
    collapsed: { remote: true, tags: true },
  };
}

/** The remembered view choices. */
export class ViewsState {
  current = $state<Views>(read());

  set<K extends keyof Views>(key: K, value: Views[K]): void {
    this.current = { ...this.current, [key]: value };
    write(this.current);
  }

  /** Opens or closes one sidebar section. */
  setCollapsed(section: string, closed: boolean): void {
    this.set('collapsed', { ...this.current.collapsed, [section]: closed });
  }

  /** Back to the shipped choices, for when something has been left unusable. */
  reset(): void {
    this.current = defaults();
    write(this.current);
  }
}

/**
 * Reads the stored choices, keeping only the ones that are still recognised.
 *
 * A value written by an older release may name a mode that no longer exists, and taking it at
 * face value would leave a panel rendering nothing.
 */
function read(): Views {
  const out = defaults();
  try {
    const raw = localStorage.getItem(KEY);
    if (raw === null) return out;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return out;
    const stored = parsed as Partial<Record<keyof Views, unknown>>;

    if (stored.density === 'compact' || stored.density === 'default'
        || stored.density === 'comfortable') {
      out.density = stored.density;
    }
    if (stored.commitFiles === 'path' || stored.commitFiles === 'tree') {
      out.commitFiles = stored.commitFiles;
    }
    if (stored.changes === 'path' || stored.changes === 'tree') out.changes = stored.changes;
    if (stored.diff === 'inline' || stored.diff === 'split') out.diff = stored.diff;
    // `fileView` is deliberately not restored. Blame and history are things you go and look
    // at, not a way you want every file opened from the next launch onwards.
    if (typeof stored.ignoreWhitespace === 'boolean') out.ignoreWhitespace = stored.ignoreWhitespace;
    // Written as a boolean until the rail existed, and a release that has been used has that
    // in storage: `false` there means the panel was deliberately put away, and losing that is
    // worse than the panel being a state behind.
    if (typeof stored.sidebar === 'boolean') out.sidebar = stored.sidebar ? 'open' : 'hidden';
    if (stored.sidebar === 'open' || stored.sidebar === 'rail' || stored.sidebar === 'hidden') {
      out.sidebar = stored.sidebar;
    }
    if (typeof stored.details === 'boolean') out.details = stored.details;
    if (typeof stored.toolbar === 'boolean') out.toolbar = stored.toolbar;
    if (typeof stored.systemTitleBar === 'boolean') out.systemTitleBar = stored.systemTitleBar;
    if (stored.terminalDock === 'bottom' || stored.terminalDock === 'right') {
      out.terminalDock = stored.terminalDock;
    }
    if (typeof stored.terminalShell === 'string') {
      out.terminalShell = stored.terminalShell;
    }
    if (typeof stored.terminalLogin === 'boolean' || stored.terminalLogin === null) {
      out.terminalLogin = stored.terminalLogin;
    }
    if (typeof stored.terminalSize === 'number' && Number.isFinite(stored.terminalSize)) {
      out.terminalSize = Math.min(900, Math.max(120, Math.round(stored.terminalSize)));
    }
    if (typeof stored.collapsed === 'object' && stored.collapsed !== null) {
      const sections: Record<string, boolean> = {};
      for (const [name, closed] of Object.entries(stored.collapsed)) {
        if (typeof closed === 'boolean') sections[name] = closed;
      }
      out.collapsed = sections;
    }
    return out;
  } catch {
    // A webview with storage disabled, or a corrupt entry, must still open the window.
    return out;
  }
}

function write(views: Views): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(views));
  } catch {
    // Losing a preference is not worth failing over.
  }
}
