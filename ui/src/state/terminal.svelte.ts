import {
  terminalClose,
  terminalListen,
  terminalOpen,
  terminalResize,
  terminalWrite,
} from '../ipc/terminal';

import type { Opened } from '../ipc/terminal';
import type { Dock, ViewsState } from './views.svelte';
import { messageOf } from '../ipc/error';

export type { Dock };

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
  error = $state<string | null>(null);

  /**
   * The shell running in each repository, by its path.
   *
   * A window-wide shell was one shell, started in whichever repository was open first, and
   * every other tab's terminal was then sitting in that checkout: `git status` in the beta tab
   * answered about alpha. One per path, so the pane a tab shows is a shell in that tab's
   * repository.
   */
  #shells = new Map<string, Opened>();

  /**
   * Where it sits and how big it is, held with the window's other view choices rather than in
   * a store of this module's own: they are all the same kind of preference and all belong to
   * the person at the window.
   */
  #views: ViewsState;

  constructor(views: ViewsState) {
    this.#views = views;
  }

  // Getters rather than `$derived` fields: a field initialiser runs before the constructor
  // body, so it would read `#views` before there is one. Reading through the getter is just as
  // reactive.
  get dock(): Dock {
    return this.#views.current.terminalDock;
  }

  /** Height when docked at the bottom, width when docked to the right. */
  get size(): number {
    return this.#views.current.terminalSize;
  }

  setDock(dock: Dock): void {
    this.#views.set('terminalDock', dock);
    // The two docks are different axes; a height that was right at the bottom is a silly
    // width at the side.
    this.#views.set('terminalSize', dock === 'right' ? 420 : 260);
  }

  setSize(px: number): void {
    this.#views.set('terminalSize', Math.min(900, Math.max(120, Math.round(px))));
  }

  toggle(): void {
    this.open = !this.open;
  }

  /** Starts a shell in `path`, or hands back the one already running there. */
  async start(path: string, cols: number, rows: number): Promise<Opened | null> {
    const running = this.#shells.get(path);
    if (running) return running;
    this.error = null;
    try {
      const opened = await terminalOpen(path, cols, rows);
      this.#shells.set(path, opened);
      return opened;
    } catch (e) {
      this.error = messageOf(e);
      return null;
    }
  }

  /**
   * Ends every shell whose repository is no longer open in a tab.
   *
   * A shell outlives the pane that shows it, on purpose — moving between tabs must not throw
   * away a half-typed command. It must not outlive the tab itself, though: those shells are
   * invisible, unreachable, and hold a pseudo-terminal each.
   */
  async keepOnly(paths: Set<string>): Promise<void> {
    const gone = [...this.#shells.keys()].filter((p) => !paths.has(p));
    await Promise.all(gone.map((p) => this.stop(p)));
  }

  /** Ends the shell in `path`, for one that exited or a tab that is closing. */
  async stop(path: string): Promise<void> {
    const running = this.#shells.get(path);
    this.#shells.delete(path);
    if (running) await terminalClose(running.id);
  }
}

export { terminalListen, terminalResize, terminalWrite };
