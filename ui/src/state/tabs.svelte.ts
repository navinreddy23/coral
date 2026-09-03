import { invoke } from '@tauri-apps/api/core';

export type GroupColour =
  | 'lane1' | 'lane2' | 'lane3' | 'lane4'
  | 'lane5' | 'lane6' | 'lane7' | 'lane8';

export interface Tab {
  id: number;
  path: string;
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

  async refresh(): Promise<void> {
    await this.#run(() => invoke<Session>('session_get'));
  }

  async open(path: string): Promise<void> {
    await this.#run(() => invoke<Session>('tab_open', { path }));
  }

  async close(id: number): Promise<void> {
    await this.#run(() => invoke<Session>('tab_close', { id }));
  }

  async activate(id: number): Promise<void> {
    await this.#run(() => invoke<Session>('tab_activate', { id }));
  }

  async group(name: string, ids: number[]): Promise<void> {
    await this.#run(() => invoke<Session>('tab_group', { name, ids }));
  }

  async ungroup(id: number): Promise<void> {
    await this.#run(() => invoke<Session>('tab_ungroup', { id }));
  }

  async setCollapsed(id: number, collapsed: boolean): Promise<void> {
    await this.#run(() => invoke<Session>('group_collapse', { id, collapsed }));
  }

  async #run(action: () => Promise<Session>): Promise<void> {
    this.error = null;
    try {
      this.session = await action();
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }
}
