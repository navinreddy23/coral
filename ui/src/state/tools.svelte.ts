import {
  openDifftool,
  openInEditor,
  openMergetool,
  toolsRead,
  toolsSetEditor,
  type DiffOf,
  type ToolsView,
} from '../ipc/commands';
import { messageOf } from '../ipc/error';

/**
 * The programs outside Coral it can hand a file to: the editor the user named, and the diff
 * and merge tools git is configured with in the open repository.
 *
 * Each `open…` answers with a sentence when it could not start one, for the window to show,
 * and null when it did.
 */
export class ToolsState {
  view = $state<ToolsView | null>(null);
  error = $state<string | null>(null);

  #path: string | null = null;

  /** Whether git has a diff tool here to start. */
  diff = $derived(this.view?.external.diff ?? null);
  /** Whether git has a merge tool here to start. */
  merge = $derived(this.view?.external.merge ?? null);

  async load(path: string | null): Promise<void> {
    this.#path = path;
    try {
      this.view = await toolsRead(path);
      this.error = null;
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async setEditor(editor: string): Promise<void> {
    try {
      const settings = await toolsSetEditor(editor);
      if (this.view !== null) this.view = { ...this.view, settings };
      this.error = null;
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async edit(file: string, line: number | null = null): Promise<string | null> {
    return this.#start(() => openInEditor(this.#path ?? '', file, line));
  }

  async diffTool(file: string, of: DiffOf): Promise<string | null> {
    return this.#start(() => openDifftool(this.#path ?? '', file, of));
  }

  async mergeTool(file: string): Promise<string | null> {
    return this.#start(() => openMergetool(this.#path ?? '', file));
  }

  async #start(launch: () => Promise<void>): Promise<string | null> {
    if (this.#path === null) return 'No repository is open.';
    try {
      await launch();
      return null;
    } catch (e) {
      return messageOf(e);
    }
  }
}
