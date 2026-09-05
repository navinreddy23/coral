import { graphScope, setGraphScope, type RepoScope } from '../ipc/commands';
import { messageOf } from '../ipc/error';

export type { RepoScope };

/**
 * Which branches and tags this repository's graph is walked from.
 *
 * Held per repository and persisted in Rust rather than in `localStorage`, unlike the rest of
 * the window's preferences: hiding a spike belongs to the repository it is a spike in, not to
 * the person who happens to have it open. `views.svelte.ts` states the same rule from the
 * other side.
 *
 * Solo and hidden are kept apart rather than collapsed into one list, because leaving solo has
 * to give the hidden branches back.
 */
export class ScopeState {
  /** The ref being soloed, by full name. */
  solo = $state<string | null>(null);
  /** Refs left out of the walk, by full name. */
  hidden = $state<string[]>([]);
  error = $state<string | null>(null);

  /** True when the graph is showing everything, which is the case worth not announcing. */
  showingAll = $derived(this.solo === null && this.hidden.length === 0);

  /** Whether `name` is one the graph is currently walked from. */
  walks(name: string): boolean {
    return this.solo === null ? !this.hidden.includes(name) : this.solo === name;
  }

  /**
   * Whether `name` was hidden by hand.
   *
   * Not the same question as [`walks`], and the graph's labels need this one: soloing leaves
   * every other ref unwalked, but a tag on a commit the soloed branch reaches is still a tag
   * worth drawing. Only a ref the user actually hid should lose its label.
   */
  hides(name: string): boolean {
    // Solo wins, exactly as it does in the engine, where `Tips::Only` never consults the
    // hidden list. A scope carrying both for one ref can still arrive from a hand-edited
    // `scope.json`, and the soloed branch must keep its label whatever else the file says.
    return this.solo !== name && this.hidden.includes(name);
  }

  /** Which repository is wanted, so an answer for the one being left can be dropped. */
  #path = '';

  async load(path: string): Promise<void> {
    this.#path = path;
    try {
      const held = await graphScope(path);
      if (this.#path !== path) return;
      this.take(held);
    } catch (e) {
      if (this.#path !== path) return;
      // A repository whose scope cannot be read still has to open, showing everything.
      this.error = messageOf(e);
      this.solo = null;
      this.hidden = [];
    }
  }

  /**
   * Solos `name`, or leaves solo when it is already the one being soloed.
   *
   * One at a time: soloing a second branch moves solo to it rather than adding it, so the
   * banner always has one name to give and one thing to undo.
   */
  async setSolo(path: string, name: string | null): Promise<void> {
    const next = name !== null && name === this.solo ? null : name;
    await this.write(path, { solo: next, hidden: this.hidden });
  }

  /**
   * Takes `name` out of the walk, or puts it back.
   *
   * Hiding the branch being soloed leaves solo as well, because the two together say opposite
   * things about the same ref and the window had no way to draw that: the banner named the
   * branch as the only one shown, the panel struck its eye through, and the graph drew its
   * commits with its own label missing. "Hide this" is the later instruction, so it wins.
   */
  async toggleHidden(path: string, name: string): Promise<void> {
    const going = !this.hidden.includes(name);
    const hidden = going ? [...this.hidden, name] : this.hidden.filter((n) => n !== name);
    const solo = going && this.solo === name ? null : this.solo;
    await this.write(path, { solo, hidden });
  }

  /** Back to the whole graph, for the button on the banner. */
  async showEverything(path: string): Promise<void> {
    await this.write(path, { solo: null, hidden: [] });
  }

  /**
   * The least change that puts `name` back in the walk.
   *
   * Leaves solo, and unhides that one ref if it was hidden. Deliberately not
   * {@link showEverything}: clicking one row asks for that row, and handing back every branch
   * somebody hid on purpose is a bigger answer than the question.
   */
  async reveal(path: string, name: string): Promise<void> {
    await this.write(path, { solo: null, hidden: this.hidden.filter((n) => n !== name) });
  }

  /** Empties the panel, for a repository being left. */
  clear(): void {
    this.#path = '';
    this.solo = null;
    this.hidden = [];
    this.error = null;
  }

  /**
   * Stores a scope and takes back what was actually kept.
   *
   * What comes back, not what went out: Rust drops any name the repository no longer has, and
   * a banner naming a branch that was deleted this morning is worse than no banner.
   */
  private async write(path: string, scope: RepoScope): Promise<void> {
    this.#path = path;
    try {
      const kept = await setGraphScope(path, scope);
      if (this.#path !== path) return;
      this.take(kept);
      this.error = null;
    } catch (e) {
      if (this.#path !== path) return;
      this.error = messageOf(e);
    }
  }

  /**
   * Takes what came back over IPC.
   *
   * Coerced rather than assigned, because this is the boundary: an answer missing a field
   * gives `undefined`, which is not `null` and so reads as a branch being soloed whose name is
   * nothing at all. The panel then tries to shorten it and the window goes down.
   */
  private take(scope: RepoScope | undefined): void {
    this.solo = scope?.solo ?? null;
    this.hidden = scope?.hidden ?? [];
  }
}
