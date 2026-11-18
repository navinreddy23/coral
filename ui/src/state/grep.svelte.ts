import { codeOf, messageOf } from '../ipc/error';
import { grepCode, stopGrep } from '../ipc/grep';
import type { GrepMatch, GrepPattern, GrepQuery, GrepResult } from '../ipc/types';

/** Where a code search looks: a repository at one revision, and the name it goes by on screen. */
export interface GrepAt {
  repo: string;
  rev: string;
  label: string;
}

/** One file's lines that matched, in the order git found them. */
export interface GrepFile {
  path: string;
  matches: GrepMatch[];
}

/**
 * The code search panel: what it searches, what for, and what came back.
 *
 * Shaped like the find box's state, and for the same reasons. A search runs a moment after the
 * typing stops, the latest one wins, and the engine stops the one before it, so a phrase typed
 * with pauses in it costs one `git grep` of the kernel rather than one per pause.
 */
export class GrepState {
  at = $state<GrepAt | null>(null);
  pattern = $state('');
  how = $state<GrepPattern>('fixed');
  ignoreCase = $state(true);
  /** Where to look, as pathspecs separated by spaces; empty for the whole tree. */
  paths = $state('');
  result = $state.raw<GrepResult | null>(null);
  /** What `result` answers, which is not what is typed once typing has moved on. */
  asked = $state.raw<GrepQuery | null>(null);
  searching = $state(false);
  error = $state<string | null>(null);

  /** The answer by file, which is how a reader goes through it. */
  files = $derived(filesOf(this.result));

  #token = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  /** The repository searched last, which outlives the panel being closed. */
  #repo: string | null = null;

  /**
   * Opens the panel on a revision, searching it for what was typed last if there is any.
   *
   * The pattern and how it is read are kept, as a search box keeps them; the paths are not,
   * once the repository is another, since one repository's paths say nothing about another's
   * and a search narrowed to `kernel` found nothing in a repository without one.
   */
  open(at: GrepAt): void {
    const same = this.at !== null && this.at.repo === at.repo && this.at.rev === at.rev;
    if (this.#repo !== null && this.#repo !== at.repo) this.paths = '';
    this.#repo = at.repo;
    this.at = at;
    if (same) return;
    this.result = null;
    this.error = null;
    if (this.pattern !== '') this.#soon();
  }

  close(): void {
    this.stop();
    this.at = null;
    this.result = null;
    this.error = null;
  }

  /** Changes what is searched for or how, and searches again once the typing stops. */
  set(change: Partial<{ pattern: string; how: GrepPattern; ignoreCase: boolean; paths: string }>): void {
    if (change.pattern !== undefined) this.pattern = change.pattern;
    if (change.how !== undefined) this.how = change.how;
    if (change.ignoreCase !== undefined) this.ignoreCase = change.ignoreCase;
    if (change.paths !== undefined) this.paths = change.paths;
    if (this.pattern === '') {
      this.stop();
      this.result = null;
      this.error = null;
      return;
    }
    this.#soon();
  }

  #soon(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.searching = true;
    this.#timer = setTimeout(() => void this.run(), 250);
  }

  /** Searches now, for Enter and for the tests. */
  async run(): Promise<void> {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    const at = this.at;
    if (at === null || this.pattern === '') return;
    const token = ++this.#token;
    this.searching = true;
    this.error = null;
    const query: GrepQuery = {
      pattern: this.pattern,
      how: this.how,
      ignoreCase: this.ignoreCase,
      paths: this.paths.split(/\s+/u).filter((p) => p !== ''),
    };
    try {
      const found = await grepCode(at.repo, at.rev, query);
      if (token !== this.#token) return;
      this.result = found;
      this.asked = query;
    } catch (e) {
      // A newer search stopped this one, and that search answers instead.
      if (token !== this.#token || codeOf(e) === 'cancelled') return;
      this.error = messageOf(e);
      this.result = null;
    } finally {
      if (token === this.#token) this.searching = false;
    }
  }

  /** Stops the search running, keeping what was typed. */
  stop(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#token += 1;
    if (this.searching && this.at !== null) void stopGrep(this.at.repo).catch(() => false);
    this.searching = false;
  }
}

function filesOf(result: GrepResult | null): GrepFile[] {
  const out: GrepFile[] = [];
  for (const match of result?.matches ?? []) {
    const last = out[out.length - 1];
    if (last !== undefined && last.path === match.path) last.matches.push(match);
    else out.push({ path: match.path, matches: [match] });
  }
  return out;
}
