import { fileBlame, fileDiff, fileHistory, fileText, worktreeDiff } from '../ipc/commands';
import type { Blame, Commit, FileDiff } from '../ipc/types';
import type { DiffMode, FileView, ViewsState } from './views.svelte';
import { messageOf } from '../ipc/error';

export type { DiffMode, FileView };

/** How many commits a page of a file's history holds. */
export const HISTORY_PAGE = 50;

/** Where the diff on screen came from, so it can be read again. */
interface Request {
  repo: string;
  source: 'commit' | 'unstaged' | 'staged';
  /** The commit, for a commit diff. */
  rev: string;
  path: string;
}

/** The file currently open in the diff viewer. */
export class DiffState {
  file = $state<FileDiff | null>(null);
  /** Path being shown, held separately so the header has something during the load. */
  path = $state<string | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  /** Who last changed each line, once the blame view has asked for it. */
  blame = $state<Blame | null>(null);
  /** The file itself at the revision the blame was taken at, so the two line up. */
  text = $state<string | null>(null);
  /** The commits that touched this file, newest first, once the history view has asked. */
  history = $state<Commit[]>([]);
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
    const before = wholeFileFor(this.mode);
    this.#views.set('diff', mode);
    if (wholeFileFor(mode) !== before) void this.#reread();
  }

  /** The change, who wrote each line, or what has touched the file. Remembered like the mode. */
  get view(): FileView {
    return this.#views.current.fileView;
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
    if (this.view === 'blame') await this.#loadBlame();
    else if (this.view === 'history') await this.#loadHistory();
  }

  async #loadBlame(): Promise<void> {
    const request = this.#request;
    if (request === null) return;
    const side = ++this.#side;
    this.blame = null;
    this.text = null;
    // A commit's blame is of the file as that commit left it; the working tree's is of HEAD,
    // since a line nobody has committed has nobody to attribute it to.
    const rev = request.source === 'commit' ? request.rev : 'HEAD';
    try {
      const [blame, text] = await Promise.all([
        fileBlame(request.repo, rev, request.path),
        fileText(request.repo, rev, request.path),
      ]);
      if (side !== this.#side) return;
      this.blame = blame;
      this.text = text;
    } catch (e) {
      if (side === this.#side) this.error = messageOf(e);
    }
  }

  async #loadHistory(): Promise<void> {
    const request = this.#request;
    if (request === null) return;
    const side = ++this.#side;
    try {
      const got = await fileHistory(request.repo, request.path, this.#historyLimit);
      if (side !== this.#side) return;
      this.history = got;
      this.moreHistory = got.length >= this.#historyLimit;
    } catch (e) {
      if (side === this.#side) this.error = messageOf(e);
    }
  }

  /** What is being shown, so the header can say whether it is a commit or the working tree. */
  source = $state<'commit' | 'unstaged' | 'staged'>('commit');

  /** What the panel was opened on, so the history can hand it back. */
  #opened: Request | null = null;

  /** Opens one file's diff from a commit. A second call supersedes the first. */
  async open(repo: string, rev: string, path: string): Promise<void> {
    await this.#load(
      { repo, source: 'commit', rev, path },
      'This commit did not change that file.',
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
      { repo, source: staged ? 'staged' : 'unstaged', rev: '', path },
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
      const got = await read(request, wholeFileFor(this.mode), this.ignoreWhitespace);
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
    this.history = [];
    this.moreHistory = false;
    this.source = request.source;
    this.path = request.path;
    this.file = null;
    this.error = null;
    this.loading = true;
    try {
      const got = await read(request, wholeFileFor(this.mode), this.ignoreWhitespace);
      // Clicking down a long file list must not let an earlier, slower read win.
      if (token !== this.#token) return;
      this.file = got;
      if (got === null) this.error = absent;
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
    this.loading = false;
    this.source = 'commit';
    this.#opened = null;
    this.blame = null;
    this.text = null;
    this.history = [];
    this.moreHistory = false;
    this.atCommit = null;
  }
}

/** Side by side shows the file end to end; inline shows the hunks. */
function wholeFileFor(mode: DiffMode): boolean {
  return mode === 'split';
}

function read(request: Request, wholeFile: boolean, ignoreWhitespace: boolean): Promise<FileDiff | null> {
  return request.source === 'commit'
    ? fileDiff(request.repo, request.rev, request.path, wholeFile, ignoreWhitespace)
    : worktreeDiff(
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
  return source === 'commit' ? 'This commit did not change that file.' : absent(source === 'staged');
}
