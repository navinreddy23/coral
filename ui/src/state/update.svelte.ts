import { messageOf } from '../ipc/error';
import {
  checkForUpdate,
  installUpdate,
  onUpdateProgress,
  type UpdateCheck,
  type UpdateProgress,
} from '../ipc/update';

const STORAGE_KEY = 'coral.updates.automatic';

/**
 * Whether a newer Coral is out, and installing it.
 *
 * Nothing is asked of GitHub unless someone presses the button in About, or has turned on the
 * check at launch, which is off until they do: the window contacts nothing on its own.
 */
export class UpdateState {
  /** The last answer, or null before anything has been asked. */
  result = $state<UpdateCheck | null>(null);
  checking = $state(false);
  installing = $state(false);
  progress = $state<UpdateProgress | null>(null);
  error = $state<string | null>(null);
  /** Whether to check at launch. */
  automatic = $state(false);

  constructor() {
    try {
      this.automatic = localStorage.getItem(STORAGE_KEY) === 'true';
    } catch {
      this.automatic = false;
    }
  }

  /** True when a newer release has been found. */
  get available(): boolean {
    return this.result?.newer === true;
  }

  /** How much of the download has arrived, or null while that is not known. */
  get fraction(): number | null {
    const p = this.progress;
    return p === null || p.total === null || p.total === 0 ? null : p.received / p.total;
  }

  setAutomatic(on: boolean): void {
    this.automatic = on;
    try {
      localStorage.setItem(STORAGE_KEY, String(on));
    } catch {
      // A browser that refuses storage just asks again next launch.
    }
  }

  async check(): Promise<UpdateCheck | null> {
    this.checking = true;
    this.error = null;
    try {
      this.result = await checkForUpdate();
      return this.result;
    } catch (e) {
      this.error = messageOf(e);
      return null;
    } finally {
      this.checking = false;
    }
  }

  /**
   * The check at launch, when it has been turned on.
   *
   * A failure stays quiet: nobody asked, and a network that is down at startup is not news
   * worth putting in front of them.
   */
  async atLaunch(): Promise<UpdateCheck | null> {
    if (!this.automatic) return null;
    const found = await this.check();
    this.error = null;
    return found;
  }

  /** Installs the newer release and restarts into it; on success this never returns. */
  async install(): Promise<void> {
    this.installing = true;
    this.error = null;
    this.progress = null;
    const stop = await onUpdateProgress((p) => (this.progress = p));
    try {
      await installUpdate();
    } catch (e) {
      this.error = messageOf(e);
    } finally {
      stop();
      this.installing = false;
    }
  }
}
