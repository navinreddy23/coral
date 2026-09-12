import {
  compareFileDiff,
  fileBlame,
  fileDiff,
  fileHistory,
  fileText,
  worktreeDiff,
} from '../ipc/commands';
import type { Blame, Commit, FileDiff } from '../ipc/types';
import type { DiffMode, FileView, ViewsState } from './views.svelte';
import { messageOf } from '../ipc/error';

export type { DiffMode, FileView };

/** How many commits a page of a file's history holds. */
export const HISTORY_PAGE = 50;

/** Where the diff on screen came from, so it can be read again. */
interface Request {
  repo: string;
  source: 'commit' | 'unstaged' | 'staged' | 'compare';
  /** The commit, for a commit diff; the older of the two when comparing. */
  rev: string;
  /** The newer of the two, when comparing. */
  to: string;
  path: string;
  /**
   * The name the file had before, when this change renamed it.
   *
   * Asked for alongside the new name, because git sees a rename by pairing a deletion with an
   * addition: given the new name alone it has nothing to pair, and reports the whole file as
   * added. A file moved with a one-line edit came out as every line of it.
   */
  oldPath: string | null;
}

/** The file currently open in the diff viewer. */
export class DiffState {
  file = $state<FileDiff | null>(null);
  /** Path being shown, held separately so the header has something during the load. */
  path = $state<string | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  /**
   * What went wrong reading the blame or the history, which is not what went wrong reading the
   * diff.
   *
   * Kept apart because they are different questions about the same file and either can fail on
   * its own. Sharing one field, a blame git refused — of a submodule pointer, say — left the
   * blame pane on "Working out who wrote each line…" for ever, because that pane asks whether
   * the answer has arrived and never whether it can, and put git's complaint under the diff
   * instead, where nothing had gone wrong.
   */
  sideError = $state<string | null>(null);

  /** Who last changed each line, once the blame view has asked for it. */
  blame = $state<Blame | null>(null);
  /** The file itself at the revision the blame was taken at, so the two line up. */
  text = $state<string | null>(null);
  /**
   * The commits that touched this file, newest first, once the history view has asked.
   *
   * Null until the read comes back, which is not the same as empty. Walking a kernel file's
   * history takes ten seconds, and for all of them the pane said "Nothing has touched this
   * file" — which is a sentence a reader believes.
   */
  history = $state<Commit[] | null>(null);
  /** True while there may be older commits than the ones held. */
  moreHistory = $state(false);
  /** The commit whose change to this file is being shown, when it came from the history. */
  atCommit = $state<string | null>(null);

  #token = 0;
  #side = 0;
  #views: ViewsState;
  #request: Request | null = null;
  #historyLimit = HISTORY_PAGE;

  constructor(views: ViewsState) {
    this.#views = views;
  }

  /**
   * Inline or side by side, remembered across launches.
   *
   * Nobody picks side-by-side once and means it for one file. A getter rather than a `$derived`
   * field, because a field initialiser runs before the constructor body.
   */
  get mode(): DiffMode {
    return this.#views.current.diff;
  }

  /**
   * Changes the layout, and re-reads the file if that changes how much of it is wanted.
   *
   * Side by side shows the whole file, which git only supplies if it is asked to; the hunks a
   * unified view is built from are a window cut out of it and there is nothing to widen them
   * with on this side. What is on screen stays up until the new answer lands, so the panel does
   * not blank on a button press.
   */
  setMode(mode: DiffMode): void {
    const before = this.wholeFile;
    this.#views.set('diff', mode);
    if (this.wholeFile !== before) void this.#reread();
  }

  /**
   * Whether the unified view shows the file around the changes rather than only the hunks.
   *
   * Not remembered between files. Widening the context is a thing somebody does to read one
   * change in its surroundings, and coming back to the next file with every line of it on
   * screen is not what they asked for.
   */
  expanded = $state(false);

  /** Shows the whole file in the unified view, or goes back to the hunks. */
  setExpanded(expanded: boolean): void {
    const before = this.wholeFile;
    this.expanded = expanded;
    if (this.wholeFile !== before) void this.#reread();
  }

  /** How much of the file to ask git for, which the layout and the toggle decide together. */
  wholeFile = $derived(wholeFileFor(this.mode) || this.expanded);

  /**
   * The change, who wrote each line, or what has touched the file. Remembered like the mode.
   *
   * Except for a comparison of two commits, which is always the change itself: blame and
   * history are about one file's past, and opening a comparison while the panel was left on
   * the history tab showed a file's history beside a range's diff, with nothing selected in
   * the list. And except for a binary file, which has no lines to attribute: the pane painted
   * four kilobytes of replacement characters for one. The remembered choice is left alone in
   * both cases, so it comes back with the next file.
   */
  get view(): FileView {
    if (this.source === 'compare') return 'diff';
    const remembered = this.#views.current.fileView;
    return remembered === 'blame' && this.file?.binary === true ? 'diff' : remembered;
  }

  setView(view: FileView): void {
    if (view === this.view) return;
    this.#views.set('fileView', view);
    void this.#sideLoad();
  }

  /** Whether a change that is only whitespace counts as one. */
  get ignoreWhitespace(): boolean {
    return this.#views.current.ignoreWhitespace;
  }

  setIgnoreWhitespace(ignore: boolean): void {
    if (ignore === this.ignoreWhitespace) return;
    this.#views.set('ignoreWhitespace', ignore);
    void this.#reread();
  }

  /** Shows what one commit from the history did to this file. */
  async showCommit(oid: string): Promise<void> {
    const request = this.#request;
    if (request === null) return;
    this.atCommit = oid;
    await this.#reread({ ...request, source: 'commit', rev: oid });
  }

  /** Back to the change the panel was opened on. */
  async showOpened(): Promise<void> {
    const request = this.#opened;
    if (request === null) return;
    this.atCommit = null;
    await this.#reread(request);
  }

  /** Another page of history, oldest-ward. */
  async deeper(): Promise<void> {
    this.#historyLimit += HISTORY_PAGE;
    await this.#loadHistory();
  }

  /**
   * Reads whatever the current view needs beyond the diff itself.
   *
   * Blame and history are each a walk of the file's whole history and cost seconds on a large
   * repository, so neither is read until the view that shows it is asked for.
   */
  async #sideLoad(): Promise<void> {
    this.sideError = null;
    if (this.view === 'blame') await this.#loadBlame();
    else if (this.view === 'history') await this.#loadHistory();
  }

  async #loadBlame(): Promise<void> {
    const request = this.#request;
    if (request === null) return;
    const side = ++this.#side;
    this.blame = null;
    this.text = null;
    this.sideError = null;
    // A commit's blame is of the file as that commit left it; the working tree's is of HEAD,
    // since a line nobody has committed has nobody to attribute it to.
    const rev = revisionOf(request);
    try {
      const [blame, text] = await Promise.all([
        fileBlame(request.repo, rev, request.path),
        fileText(request.repo, rev, request.path),
      ]);
      if (side !== this.#side) return;
      this.blame = blame;
      this.text = text;
    } catch (e) {
      if (side === this.#side) this.sideError = messageOf(e);
    }
  }

  async #loadHistory(): Promise<void> {
    const request = this.#request;
    if (request === null) return;
    const side = ++this.#side;
    this.history = null;
    try {
      // From the commit being looked at, so the list holds the change on screen.
      const rev = revisionOf(request);
      const got = await fileHistory(request.repo, rev, request.path, this.#historyLimit);
      if (side !== this.#side) return;
      this.history = got;
      this.moreHistory = got.length >= this.#historyLimit;
    } catch (e) {
      if (side === this.#side) this.sideError = messageOf(e);
    }
  }

  /** What is being shown, so the header can say whether it is a commit or the working tree. */
  source = $state<'commit' | 'unstaged' | 'staged' | 'compare'>('commit');

  /** What the panel was opened on, so the history can hand it back. */
  #opened: Request | null = null;

  /** Opens one file's diff from a commit. A second call supersedes the first. */
  async open(repo: string, rev: string, path: string, oldPath: string | null = null): Promise<void> {
    await this.#load(
      { repo, source: 'commit', rev, to: '', path, oldPath },
      'This commit did not change that file.',
    );
  }

  /** Opens one file's diff between two commits. */
  async openCompare(
    repo: string,
    from: string,
    to: string,
    path: string,
    oldPath: string | null = null,
  ): Promise<void> {
    await this.#load(
      { repo, source: 'compare', rev: from, to, path, oldPath },
      'That file is the same in both commits.',
    );
  }

  /**
   * Opens one file's diff in the working tree.
   *
   * A file can be in both lists with different hunks — part of it staged, part not — so which
   * side was clicked decides which diff is shown.
   */
  async openWorking(repo: string, staged: boolean, path: string): Promise<void> {
    await this.#load(
      { repo, source: staged ? 'staged' : 'unstaged', rev: '', to: '', path, oldPath: null },
      absent(staged),
    );
  }

  /**
   * Re-reads the working-tree diff on screen, for a file that changed underneath it.
   *
   * Keeps what is showing until the new answer arrives, unlike opening one: this runs whenever
   * the working tree moves, and blanking the panel first would make an editor's autosave flash
   * it. A commit's diff is not re-read, because a commit does not change.
   */
  async reload(repo: string): Promise<void> {
    const request = this.#request;
    if (request === null || request.source === 'commit') return;
    await this.#reread({ ...request, repo });
  }

  /** Reads the open file again, leaving what is on screen up until the answer arrives. */
  async #reread(request: Request | null = this.#request): Promise<void> {
    if (request === null) return;
    this.#request = request;
    const token = ++this.#token;
    try {
      const got = await read(request, this.wholeFile, this.ignoreWhitespace);
      if (token !== this.#token) return;
      this.file = got;
      this.error = got === null ? absentFor(request.source) : null;
    } catch (e) {
      if (token !== this.#token) return;
      this.error = messageOf(e);
    }
  }

  async #load(request: Request, absent: string): Promise<void> {
    const token = ++this.#token;
    this.#request = request;
    this.#opened = request;
    this.#historyLimit = HISTORY_PAGE;
    this.atCommit = null;
    this.blame = null;
    this.text = null;
    this.history = null;
    this.moreHistory = false;
    this.source = request.source;
    this.path = request.path;
    this.file = null;
    this.error = null;
    this.loading = true;
    try {
      const got = await read(request, this.wholeFile, this.ignoreWhitespace);
      // Clicking down a long file list must not let an earlier, slower read win.
      if (token !== this.#token) return;
      this.file = got;
      if (got === null) {
        this.error = absent;
      } else if (got.change === 'unmerged') {
        // git has no patch for a path with conflict stages: it prints `* Unmerged path` and
        // counts nothing. Showing that as an empty diff says the file is unchanged, which is
        // the opposite of what is wrong with it.
        this.file = null;
        this.error = 'That file is conflicted. Resolve it, and the change will be here.';
      }
    } catch (e) {
      if (token !== this.#token) return;
      this.error = messageOf(e);
    } finally {
      if (token === this.#token) this.loading = false;
    }
    await this.#sideLoad();
  }

  close(): void {
    this.#token++;
    this.#request = null;
    this.file = null;
    this.path = null;
    this.error = null;
    this.sideError = null;
    this.loading = false;
    this.source = 'commit';
    this.#opened = null;
    this.blame = null;
    this.text = null;
    this.history = null;
    this.moreHistory = false;
    this.atCommit = null;
    this.expanded = false;
  }
}

/**
 * Which single revision a request is about.
 *
 * Blame and history each answer about one commit, and for a comparison that is the newer of
 * the two: the file as it ends up, which is the side being read.
 */
function revisionOf(request: Request): string {
  if (request.source === 'compare') return request.to;
  return request.source === 'commit' ? request.rev : 'HEAD';
}

/** Side by side shows the file end to end; inline shows the hunks. */
function wholeFileFor(mode: DiffMode): boolean {
  return mode === 'split';
}

function read(
  request: Request,
  wholeFile: boolean,
  ignoreWhitespace: boolean,
): Promise<FileDiff | null> {
  if (request.source === 'compare') {
    return compareFileDiff(
      request.repo,
      request.rev,
      request.to,
      request.path,
      request.oldPath,
      wholeFile,
      ignoreWhitespace,
    );
  }
  if (request.source === 'commit') {
    return fileDiff(
      request.repo,
      request.rev,
      request.path,
      request.oldPath,
      wholeFile,
      ignoreWhitespace,
    );
  }
  return worktreeDiff(
    request.repo,
    request.source === 'staged',
    request.path,
    wholeFile,
    ignoreWhitespace,
  );
}

/** What to say when the side being shown has nothing in it for that file. */
function absent(staged: boolean): string {
  return staged ? 'Nothing is staged for that file.' : 'That file has no unstaged changes.';
}

function absentFor(source: Request['source']): string {
  if (source === 'compare') return 'That file is the same in both commits.';
  if (source === 'commit') return 'This commit did not change that file.';
  return absent(source === 'staged');
}
