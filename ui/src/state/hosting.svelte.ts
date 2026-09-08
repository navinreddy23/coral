import { messageOf } from '../ipc/error';
import {
  hostingLogin,
  hostingLogout,
  hostingPullRequests,
  hostingStatus,
  type HostView,
  type PullRequest,
} from '../ipc/commands';

/**
 * The hosting provider behind the repository's remote.
 *
 * Nothing here is on the graph's critical path and every failure degrades to plain git: a
 * repository whose remote is not GitHub or GitLab, or that has no token, simply shows no
 * section rather than an error the user cannot act on.
 */
export class HostingState {
  view = $state<HostView | null>(null);
  pullRequests = $state<PullRequest[]>([]);
  loading = $state(false);
  /** A refusal worth showing — an expired token, most often — as opposed to no host at all. */
  error = $state<string | null>(null);

  #path = '';

  /** True once there is a host to talk to and a token to talk with. */
  available = $derived(this.view?.host != null && this.view.token !== 'none');

  /**
   * True when the token in use is the shared one rather than this profile's own.
   *
   * The window says so, because signing out of it signs every other profile out too.
   */
  shared = $derived(this.view?.token === 'shared');

  async load(path: string): Promise<void> {
    this.#path = path;
    this.error = null;
    this.pullRequests = [];
    try {
      this.view = await hostingStatus(path);
    } catch {
      // A repository with no remotes, or a keyring that cannot be reached. Neither is worth
      // interrupting the window for.
      this.view = null;
      return;
    }
    if (this.available) await this.refresh();
  }

  async refresh(): Promise<void> {
    if (!this.#path) return;
    this.loading = true;
    this.error = null;
    try {
      this.pullRequests = await hostingPullRequests(this.#path);
    } catch (e) {
      this.error = messageOf(e);
      this.pullRequests = [];
    } finally {
      this.loading = false;
    }
  }

  /** Stores a token for the profile at the window. Never touches the shared one. */
  async signIn(tokenValue: string): Promise<void> {
    this.error = null;
    try {
      this.view = await hostingLogin(this.#path, tokenValue);
      if (this.available) await this.refresh();
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  async signOut(): Promise<void> {
    try {
      this.view = await hostingLogout(this.#path);
      this.pullRequests = [];
    } catch (e) {
      this.error = messageOf(e);
    }
  }
}
