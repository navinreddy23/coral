import { messageOf } from '../ipc/error';
import {
  hostingChecks,
  hostingLogin,
  hostingLogout,
  hostingPullRequests,
  hostingStatus,
  type Checks,
  type HostView,
  type PullRequest,
} from '../ipc/commands';

/** How many proposals have their checks asked for: the ones a sidebar shows without scrolling. */
const CHECKED = 20;
/** How many of those asks are out at once, so twenty proposals are not twenty open requests. */
const AT_ONCE = 4;

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
  /** What each open proposal's checks came to, by number, as the answers arrive. */
  checks = $state<Record<number, Checks>>({});
  loading = $state(false);
  /** A refusal worth showing — an expired token, most often — as opposed to no host at all. */
  error = $state<string | null>(null);

  #path = '';
  #reading = 0;

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
    void this.readChecks(this.#path, this.pullRequests);
  }

  /**
   * Asks for the checks on each open proposal, a few at a time.
   *
   * After the list rather than with it: a host slow to answer for twenty commits must not keep
   * the proposals themselves off the screen. A proposal whose checks cannot be read just shows
   * none; the list already says whether the host is reachable.
   */
  async readChecks(path: string, proposals: PullRequest[]): Promise<void> {
    // Each reading is its own: a refresh after a push starts another while the first is still
    // out, and the first's late answer is about a commit that is no longer the tip.
    const reading = ++this.#reading;
    const current = () => reading === this.#reading && path === this.#path;
    this.checks = {};
    const queue = proposals
      .filter((p) => (p.state === 'open' || p.state === 'draft') && p.headSha !== '')
      .slice(0, CHECKED);
    const next = async (): Promise<void> => {
      const pr = queue.shift();
      if (pr === undefined || !current()) return;
      try {
        const found = await hostingChecks(path, pr.number, pr.headSha);
        if (current()) this.checks = { ...this.checks, [pr.number]: found };
      } catch {
        // Nothing to show for this one.
      }
      await next();
    };
    await Promise.all(Array.from({ length: AT_ONCE }, next));
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
    // The last failure was about the token being forgotten. Left up, the panel reported a
    // rejection of a token that is no longer there.
    this.error = null;
    try {
      this.view = await hostingLogout(this.#path);
      this.pullRequests = [];
    } catch (e) {
      this.error = messageOf(e);
    }
  }
}
