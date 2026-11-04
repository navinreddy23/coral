import {
  assistDraft,
  assistRead,
  assistSet,
  assistSetKey,
  type AssistView,
  type Draft,
} from '../ipc/commands';
import { messageOf } from '../ipc/error';

/**
 * Drafting a commit message with Claude, on the user's own key. Off until turned on; nothing
 * is sent until the Draft button is pressed.
 */
export class AssistState {
  view = $state<AssistView | null>(null);
  drafting = $state(false);
  error = $state<string | null>(null);

  /** Whether the commit box should offer a draft at all. */
  ready = $derived(this.view?.enabled === true && this.view.hasKey);

  async load(): Promise<void> {
    try {
      this.view = await assistRead();
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async set(enabled: boolean, model: string): Promise<void> {
    try {
      this.view = await assistSet(enabled, model);
      this.error = null;
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async setKey(key: string | null): Promise<void> {
    try {
      this.view = await assistSetKey(key);
      this.error = null;
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  /** A draft for what is staged at `path`, or null with the reason in `error`. */
  async draft(path: string): Promise<Draft | null> {
    this.drafting = true;
    this.error = null;
    try {
      return await assistDraft(path);
    } catch (e) {
      this.error = messageOf(e);
      return null;
    } finally {
      this.drafting = false;
    }
  }
}
