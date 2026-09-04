import {
  terminalClose,
  terminalListen,
  terminalOpen,
  terminalResize,
  terminalWrite,
} from '../ipc/terminal';

import type { Dock, ViewsState } from './views.svelte';

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
  id = $state<number | null>(null);
  shell = $state('');
  error = $state<string | null>(null);

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
