import { activityClear, activityLog, type ActivityEntry } from '../ipc/activity';
import { messageOf } from '../ipc/error';

/** Which log the window is showing. */
export type Channel = 'app' | 'repo';

/**
 * The activity log, as the window sees it.
 *
 * Read on demand rather than streamed. The log is a few hundred lines in the engine's memory
 * and is looked at when something has already gone wrong; pushing every entry across the IPC
 * boundary as it happens would cost something on every operation to save a fetch that happens
 * almost never.
 */
export class ActivityState {
  channel = $state<Channel>('repo');
  entries = $state<ActivityEntry[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);

  /** The repository whose log the `repo` channel shows. */
  #path = '';

  repo(path: string): void {
    this.#path = path;
  }

  async load(): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      this.entries = await activityLog(this.channel === 'repo' ? this.#path : null);
    } catch (e) {
      this.entries = [];
      this.error = messageOf(e);
    } finally {
      this.loading = false;
    }
  }

  async show(channel: Channel): Promise<void> {
    this.channel = channel;
    await this.load();
  }

  async clear(): Promise<void> {
    try {
      await activityClear(this.channel === 'repo' ? this.#path : null);
    } catch (e) {
      this.error = messageOf(e);
    }
    await this.load();
  }
}
