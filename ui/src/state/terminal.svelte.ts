import {
  terminalClose,
  terminalListen,
  terminalOpen,
  terminalResize,
  terminalWrite,
} from '../ipc/terminal';

/** Where the terminal is docked. */
export type Dock = 'bottom' | 'right';

const DOCK_KEY = 'coral.terminal.dock';
const SIZE_KEY = 'coral.terminal.size';

/**
 * The in-app terminal.
 *
 * One shell per repository, kept open while the tab is: a shell holds a working directory, a
 * half-typed command and an environment, and throwing that away whenever the panel is hidden
 * would make it useless for anything that takes more than one step.
 */
export class TerminalState {
  /**
   * Hidden until asked for, except in the browser preview, where the point is to look at it.
   * `import.meta.env.DEV` is replaced with false in a release build, so the branch is dropped.
   */
  open = $state(import.meta.env.DEV && !('__TAURI_INTERNALS__' in globalThis));
  dock = $state<Dock>(read(DOCK_KEY) === 'right' ? 'right' : 'bottom');
  /** Height when docked at the bottom, width when docked to the right. */
  size = $state(sizeOrDefault());
  id = $state<number | null>(null);
  shell = $state('');
  error = $state<string | null>(null);

  setDock(dock: Dock): void {
    this.dock = dock;
    write(DOCK_KEY, dock);
    // The two docks are different axes; a height that was right at the bottom is a silly
    // width at the side.
    this.size = dock === 'right' ? 420 : 260;
    write(SIZE_KEY, String(this.size));
  }

  setSize(px: number): void {
    this.size = Math.min(900, Math.max(120, Math.round(px)));
    write(SIZE_KEY, String(this.size));
  }

  toggle(): void {
    this.open = !this.open;
  }

  /** Starts a shell, or does nothing when one is already running. */
  async start(path: string, cols: number, rows: number): Promise<number | null> {
    if (this.id !== null) return this.id;
    this.error = null;
    try {
      const opened = await terminalOpen(path, cols, rows);
      this.id = opened.id;
      this.shell = opened.shell;
      return opened.id;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  async stop(): Promise<void> {
    const id = this.id;
    this.id = null;
    this.shell = '';
    if (id !== null) await terminalClose(id);
  }
}

export { terminalListen, terminalResize, terminalWrite };

function sizeOrDefault(): number {
  const stored = Number(read(SIZE_KEY));
  return Number.isFinite(stored) && stored > 0 ? stored : 260;
}

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    // A webview with storage disabled must still open.
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Losing the preference is not worth failing over.
  }
}
