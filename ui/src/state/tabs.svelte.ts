import { session as ipc } from '../ipc/commands';

export type GroupColour =
  | 'lane1' | 'lane2' | 'lane3' | 'lane4'
  | 'lane5' | 'lane6' | 'lane7' | 'lane8';

export interface Tab {
  id: number;
  path: string;
  /** A submodule being looked at inside this tab, relative to `path`. */
  submodule: string | null;
  group: number | null;
  missing: boolean;
}

export interface TabGroup {
  id: number;
  name: string;
  colour: GroupColour;
  collapsed: boolean;
}

export interface Session {
  tabs: Tab[];
  groups: TabGroup[];
  active: number | null;
}

/** A run of tabs sharing a group, which is how the bar draws them: one band per run. */
export interface Band {
  group: TabGroup | null;
  tabs: Tab[];
}

/**
 * The open repositories.
 *
 * Every command returns the whole session, so this holds no derived copy that could disagree
 * with what is on disk.
 */
export class TabsState {
  session = $state<Session>({ tabs: [], groups: [], active: null });
  error = $state<string | null>(null);

  active = $derived(this.session.tabs.find((t) => t.id === this.session.active) ?? null);

  /** Consecutive tabs sharing a group, in bar order. */
  bands = $derived.by(() => {
    const out: Band[] = [];
    for (const tab of this.session.tabs) {
      const last = out.at(-1);
      if (last && last.group?.id === (tab.group ?? undefined)) {
        last.tabs.push(tab);
        continue;
      }
      const group = this.session.groups.find((g) => g.id === tab.group) ?? null;
      if (last && last.group === null && group === null) last.tabs.push(tab);
      else out.push({ group, tabs: [tab] });
    }
    return out;
  });

  /** The name shown on a tab: the directory, which is what people call the repository. */
  static title(tab: Tab): string {
    const parts = tab.path.split('/').filter((p) => p.length > 0);
    return parts.at(-1) ?? tab.path;
  }

  /**
   * The directory a tab is currently showing.
   *
   * A submodule is a step into the tab rather than a tab of its own, so what the window loads
   * is not always the path the tab was opened for.
   */
  static workingPath(tab: Tab): string {
    return tab.submodule ? `${tab.path.replace(/\/+$/u, '')}/${tab.submodule}` : tab.path;
  }

  /** What the active tab is showing, which is what every panel loads from. */
  workingPath = $derived(this.active ? TabsState.workingPath(this.active) : null);

  async refresh(): Promise<void> {
    await this.#run(() => ipc.get());
  }

  async open(path: string): Promise<void> {
    await this.#run(() => ipc.open(path));
  }

  async close(id: number): Promise<void> {
    await this.#run(() => ipc.close(id));
  }

  async activate(id: number): Promise<void> {
    await this.#run(() => ipc.activate(id));
  }

  async group(name: string, ids: number[]): Promise<void> {
    await this.#run(() => ipc.group(name, ids));
  }

  /**
   * Moves a tab: into `group`, out of every group when it is null, and in front of `before`.
   *
   * The whole move in one call. Ungrouping and regrouping instead would rearrange the bar
   * twice, and a group left empty in between would be collected before the tab arrived.
   */
  async move(id: number, group: number | null, before: number | null): Promise<void> {
    await this.#run(() => ipc.move(id, group, before));
  }

  async ungroup(id: number): Promise<void> {
    await this.#run(() => ipc.ungroup(id));
  }

  /** Shows a submodule inside the tab that declares it, rather than in a tab of its own. */
  async enterSubmodule(id: number, path: string): Promise<void> {
    await this.#run(() => ipc.enterSubmodule(id, path));
  }

  async leaveSubmodule(id: number): Promise<void> {
    await this.#run(() => ipc.leaveSubmodule(id));
  }

  async rename(id: number, name: string): Promise<void> {
    await this.#run(() => ipc.rename(id, name));
  }

  async recolour(id: number, colour: GroupColour): Promise<void> {
    await this.#run(() => ipc.recolour(id, colour));
  }

  /** Dissolves a group, leaving its tabs open. */
  async dissolve(id: number): Promise<void> {
    await this.#run(() => ipc.dissolve(id));
  }

  async closeGroup(id: number): Promise<void> {
    await this.#run(() => ipc.closeGroup(id));
  }

  async setCollapsed(id: number, collapsed: boolean): Promise<void> {
    await this.#run(() => ipc.collapse(id, collapsed));
  }

  async #run(action: () => Promise<Session>): Promise<void> {
    this.error = null;
    try {
      const next = await action();
      // A malformed answer keeps what is on screen. Assigning it anyway empties the bar and
      // every read of it then throws, which loses the error along with the tabs.
      if (!next || !Array.isArray(next.tabs)) {
        throw new Error('the session could not be read');
      }
      this.session = next;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }
}
