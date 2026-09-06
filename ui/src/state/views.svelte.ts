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

export interface Views {
  /** The commit detail panel's file list. */
  commitFiles: Grouping;
  /** The staging panel's two lists. */
  changes: Grouping;
  diff: DiffMode;
  fileView: FileView;
  /** Whether a change that is only whitespace counts as a change. */
  ignoreWhitespace: boolean;
  /** Whether the left panel, the detail panel and the toolbar are showing. */
  sidebar: boolean;
  details: boolean;
  toolbar: boolean;
  terminalDock: Dock;
  terminalSize: number;
  /** Which sections of the sidebar are closed, by key. */
  collapsed: Record<string, boolean>;
}

const KEY = 'coral.views';

function defaults(): Views {
  return {
    commitFiles: 'path',
    changes: 'tree',
    diff: 'inline',
    fileView: 'diff',
    ignoreWhitespace: false,
    sidebar: true,
    details: true,
    toolbar: true,
    terminalDock: 'bottom',
    terminalSize: 260,
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

    if (stored.commitFiles === 'path' || stored.commitFiles === 'tree') {
      out.commitFiles = stored.commitFiles;
    }
    if (stored.changes === 'path' || stored.changes === 'tree') out.changes = stored.changes;
    if (stored.diff === 'inline' || stored.diff === 'split') out.diff = stored.diff;
    // `fileView` is deliberately not restored. Blame and history are things you go and look
    // at, not a way you want every file opened from the next launch onwards.
    if (typeof stored.ignoreWhitespace === 'boolean') out.ignoreWhitespace = stored.ignoreWhitespace;
    if (typeof stored.sidebar === 'boolean') out.sidebar = stored.sidebar;
    if (typeof stored.details === 'boolean') out.details = stored.details;
    if (typeof stored.toolbar === 'boolean') out.toolbar = stored.toolbar;
    if (stored.terminalDock === 'bottom' || stored.terminalDock === 'right') {
      out.terminalDock = stored.terminalDock;
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
