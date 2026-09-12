import { invoke } from './invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { GroupColour, Session, TabIcon } from '../state/tabs.svelte';
import type {
  Ancestry,
  Blame,
  Blocks,
  ChangedFile,
  Commit,
  CommitDetail,
  Todo,
  ConflictedFile,
  FileDiff,
  GitRef,
  Operation,
  OpOutcome,
  SigningConfig,
  SigningFormat,
  SigningKey,
  SigningOverrides,
  SigningScopes,
  SshConfig,
  SshKey,
  SshOverrides,
  SshScopes,
  Status,
  Submodule,
  SubmoduleRevision,
  Remote,
  RepoInfo,
  Worktree,
} from './types';

/**
 * The only module that calls `invoke`. Types come from `types.ts`, which Rust generates via
 * ts-rs; the wrappers are hand-written because the binary graph path cannot be described by
 * any generator.
 */
export async function open(path: string): Promise<RepoInfo> {
  return invoke<RepoInfo>('open_repo', { path });
}

/** Which repository to open on launch. Replaced by the tab session later in M5. */
export async function initialRepo(): Promise<string> {
  return invoke<string>('initial_repo');
}

/**
 * Asks for a repository directory.
 *
 * A native picker rather than a text box: a path typed by hand is the one input nobody gets
 * right, and the dialog also confirms the directory exists before we try to open it.
 */
export async function pickRepository(): Promise<string | null> {
  const chosen = await openDialog({
    directory: true,
    multiple: false,
    title: 'Open a repository',
  });
  return typeof chosen === 'string' ? chosen : null;
}

/** A ref together with the graph row it labels, or null when that commit is not in the walk. */
export interface PlacedRef extends GitRef {
  row: number | null;
}

/**
 * Asks for a program to run, such as a particular gpg build.
 *
 * A picker rather than a typed path for the same reason as the repository one: a path typed by
 * hand is the input nobody gets right, and the dialog confirms the file exists.
 */
export async function pickProgram(): Promise<string | null> {
  return pickFile('Choose the signing program');
}

/** Asks for a git binary, for someone whose git is somewhere Coral would not look. */
export async function pickGitProgram(): Promise<string | null> {
  return pickFile('Choose a git executable');
}

async function pickFile(title: string): Promise<string | null> {
  const chosen = await openDialog({ directory: false, multiple: false, title });
  return typeof chosen === 'string' ? chosen : null;
}

/** Every ref, each already resolved to the graph row it labels. */
export function repoRefs(path: string): Promise<PlacedRef[]> {
  return invoke<PlacedRef[]>('repo_refs', { path });
}

/**
 * Where a revision stands relative to the current branch.
 *
 * Asked when a menu opens rather than carried on every ref: it costs an ancestry walk, and
 * only the menu needs the answer.
 */
export function revAncestry(path: string, rev: string): Promise<Ancestry> {
  return invoke<Ancestry>('rev_ancestry', { path, rev });
}

/**
 * Which refs a repository's graph is walked from.
 *
 * Both by full ref name, because that is what survives: `origin/main` and a local `main` are
 * one pill in the graph and two different tips.
 */
export interface RepoScope {
  /** The one ref being soloed, or null when the graph is showing everything it is given. */
  solo: string | null;
  /** Refs left out of the walk. Kept while soloing, so leaving solo restores them. */
  hidden: string[];
}

/** What this repository is showing, with any ref it no longer has already taken out. */
export function graphScope(path: string): Promise<RepoScope> {
  return invoke<RepoScope>('graph_scope', { path });
}

/**
 * Sets what this repository shows, answering with what was actually stored.
 *
 * The answer is the pruned scope rather than nothing, so the window cannot come away believing
 * it soloed a branch that has since been deleted and then name it in the banner.
 */
export function setGraphScope(path: string, scope: RepoScope): Promise<RepoScope> {
  return invoke<RepoScope>('set_graph_scope', { path, scope });
}

/**
 * The row a commit sits on, or `null` when it is outside the graph that is loaded.
 *
 * For a commit no ref names: a detached HEAD has a row worth showing and no label to find it
 * by, and the row lookup belongs where the sorted index already is.
 */
export function graphRowOf(path: string, oid: string): Promise<number | null> {
  return invoke<number | null>('graph_row_of', { path, oid });
}

/** Every file the repository holds at one commit, for the panel's "all files" view. */
export function commitTree(path: string, rev: string): Promise<string[]> {
  return invoke<string[]>('commit_tree', { path, rev });
}

/** The repository's submodules. Empty for a repository that declares none. */
export function repoSubmodules(path: string): Promise<Submodule[]> {
  return invoke<Submodule[]>('repo_submodules', { path });
}

/**
 * The repository's working trees, its own first.
 *
 * Always at least one. A repository with only its own tree has nothing to manage, which is
 * what the panel reads this to decide.
 */
export function repoWorktrees(path: string): Promise<Worktree[]> {
  return invoke<Worktree[]>('repo_worktrees', { path });
}

/**
 * The commit a submodule is pinned at, described.
 *
 * Read from inside the submodule, because that is the only place the message lives: the
 * superproject records an object id and nothing else. Null when it has no working copy.
 */
export function submoduleRevision(
  path: string,
  submodule: string,
): Promise<SubmoduleRevision | null> {
  return invoke<SubmoduleRevision | null>('submodule_revision', { path, submodule });
}

/**
 * How to ask for the patch: how much of the file, whether whitespace counts, and whether the
 * size guard still applies.
 *
 * Mirrors `graph::ReadOptions` in Rust. One argument rather than three, because every command
 * that reads a patch carries all of them and a row of bare booleans says nothing about which
 * is which.
 */
export type ReadOptions = {
  wholeFile: boolean;
  ignoreWhitespace: boolean;
  /** False once the reader has been shown the size guard and asked for the contents anyway. */
  guardLarge: boolean;
};

/**
 * Hunks for one file in one commit, or null when the commit did not touch it.
 *
 * One file at a time: a large merge touches thousands, and their patches together are far
 * more than the panel can show or the webview should hold.
 */
export function fileDiff(
  path: string,
  rev: string,
  file: string,
  /** The name it had before, when the commit renamed it. Null otherwise. */
  oldFile: string | null,
  options: ReadOptions,
): Promise<FileDiff | null> {
  return invoke<FileDiff | null>('file_diff', { path, rev, file, oldFile, options });
}

/** Who last changed each line of a file, and in which commit. */
export function fileBlame(path: string, rev: string, file: string): Promise<Blame> {
  return invoke<Blame>('file_blame', { path, rev, file });
}

/** What to do with part of a file's changes. */
export type Part = 'stage' | 'unstage' | 'discard';

/**
 * Stages, unstages or discards one hunk of a file, or only some of its lines.
 *
 * `lines` indexes the hunk's own lines; empty means the whole hunk.
 */
export function applyPart(
  path: string,
  file: string,
  part: Part,
  hunk: number,
  lines: number[],
): Promise<Status> {
  return invoke<Status>('apply_part', { path, file, part, hunk, lines });
}

/** Deletes files outright: gone from the working tree, and staged as removed if tracked. */
export function deletePaths(
  path: string,
  tracked: string[],
  untracked: string[],
): Promise<Status> {
  return invoke<Status>('delete_paths', { path, tracked, untracked });
}

/** The files that differ between two commits, oldest first. */
export function compareCommits(path: string, from: string, to: string): Promise<ChangedFile[]> {
  return invoke<ChangedFile[]>('compare_commits', { path, from, to });
}

/** One file's diff between two commits. */
export function compareFileDiff(
  path: string,
  from: string,
  to: string,
  file: string,
  /** The name it had at `from`, when it was renamed between the two. Null otherwise. */
  oldFile: string | null,
  options: ReadOptions,
): Promise<FileDiff | null> {
  return invoke<FileDiff | null>('compare_file_diff', { path, from, to, file, oldFile, options });
}

/** One file's contents at one revision, for the blame view to put its chunks beside. */
export function fileText(path: string, rev: string, file: string): Promise<string> {
  return invoke<string>('file_text', { path, rev, file });
}

/** The commits that touched one file, newest first, following it across renames. */
export function fileHistory(
  path: string,
  rev: string,
  file: string,
  limit: number,
): Promise<Commit[]> {
  return invoke<Commit[]>('file_history', { path, rev, file, limit });
}

/**
 * One file's diff in the working tree, staged or not.
 *
 * A different question from `fileDiff`: that asks what a commit changed, this what has changed
 * since. A file can be in both answers with different hunks, which is why the staging panel has
 * two lists.
 */
export function worktreeDiff(
  path: string,
  staged: boolean,
  file: string,
  options: ReadOptions,
): Promise<FileDiff | null> {
  return invoke<FileDiff | null>('worktree_diff', { path, staged, file, options });
}

/**
 * Something to ask the repository to do.
 *
 * Mirrors `actions::Action` in Rust, which is a tagged union because every one of these
 * follows the same snapshot-run-journal path in the engine.
 */
export type Action =
  | { kind: 'fetch'; remote: string | null }
  | { kind: 'pull'; remote: string | null; mode: 'ffOnly' | 'merge' | 'rebase' }
  | {
      kind: 'push';
      remote: string | null;
      setUpstream: boolean;
      /** One ref instead of the current branch, such as `refs/tags/v1.0`. */
      refspec: string | null;
      /** Send every tag as well. Tags travel only when they are asked for. */
      tags: boolean;
      /**
       * Overwrite what is on the remote, provided nothing has moved it since it was last
       * fetched. Always `--force-with-lease`; there is no bare force anywhere in Coral.
       */
      forceWithLease: boolean;
      /** Remove the named ref from the remote instead of updating it. */
      delete: boolean;
    }
  | { kind: 'checkout'; rev: string }
  | { kind: 'branchCreate'; name: string; at: string | null; checkout: boolean }
  | { kind: 'branchDelete'; name: string; force: boolean }
  | { kind: 'branchRename'; from: string; to: string }
  | { kind: 'merge'; rev: string; mode: 'auto' | 'noFf' | 'ffOnly' | 'squash' }
  | { kind: 'rebase'; onto: string }
  | { kind: 'cherryPick'; revs: string[]; commit: boolean; mainline?: number }
  | { kind: 'revert'; revs: string[]; mainline?: number }
  | { kind: 'stashPush'; message: string | null }
  | { kind: 'stashApply'; index: number; pop: boolean }
  | { kind: 'stashDrop'; index: number }
  | { kind: 'tagCreate'; name: string; at: string | null; message: string | null }
  | { kind: 'tagDelete'; name: string }
  | { kind: 'tagMove'; name: string; at: string }
  | { kind: 'branchFastForward'; name: string; at: string }
  | { kind: 'reset'; rev: string; mode: 'soft' | 'mixed' | 'hard' }
  | { kind: 'rewrite'; rev: string; how: RewriteKind; message: string | null }
  | { kind: 'worktreeAdd'; path: string; rev: string; branch: string | null }
  | { kind: 'worktreeRemove'; path: string; force: boolean }
  | { kind: 'submoduleInit'; path: string | null; recursive: boolean; remote: boolean }
  | { kind: 'submoduleSetUrl'; path: string; url: string }
  | { kind: 'submoduleRemove'; path: string; force: boolean }
  | { kind: 'patch'; rev: string; from: string | null; directory: string }
  | { kind: 'applyPatch'; files: string[]; commit: boolean }
  | { kind: 'undo' }
  | { kind: 'redo' };

/** The history edits the commit menu offers, each an interactive rebase underneath. */
export type RewriteKind = 'drop' | 'reword' | 'moveNewer' | 'moveOlder';

export interface ActionOutcome {
  what: string;
  /** The operation stopped on conflicts and the worktree needs attention. */
  conflicted: boolean;
  /** git's own words, when there are any. "Already up to date" is the one worth showing. */
  message: string;
}

export function runAction(path: string, action: Action): Promise<ActionOutcome> {
  return invoke<ActionOutcome>('repo_action', { path, action });
}

/** The operation in progress, and what its two sides are called. */
export function repoOperation(path: string): Promise<Operation> {
  return invoke<Operation>('repo_operation', { path });
}

/** Files still needing a decision. */
export function repoConflicts(path: string): Promise<ConflictedFile[]> {
  return invoke<ConflictedFile[]>('repo_conflicts', { path });
}

/** One conflicted file broken into agreeing and disagreeing regions. */
export function conflictBlocks(path: string, file: string): Promise<Blocks> {
  return invoke<Blocks>('conflict_blocks', { path, file });
}

/** How the user chose to settle one file. */
export type Choice =
  | { kind: 'ours' }
  | { kind: 'theirs' }
  | { kind: 'delete' }
  | { kind: 'content'; text: string };

export function resolveConflict(path: string, file: string, choice: Choice): Promise<void> {
  return invoke<void>('resolve_conflict', { path, file, choice });
}

/** Continues, aborts, or skips the operation in progress. */
export function operationStep(
  path: string,
  step: 'continue' | 'abort' | 'skip',
): Promise<OpOutcome> {
  return invoke<OpOutcome>('operation_step', { path, step });
}

/** GitHub or GitLab, as identified from the remote URL. */
export interface Host {
  kind: 'github' | 'gitlab';
  origin: string;
  owner: string;
  repo: string;
}

/** Which stored token a host is reached with. */
export type TokenSource = 'none' | 'profile' | 'shared';

export interface HostView {
  host: Host | null;
  /** Why no host was identified. Not an error: plain git still works without one. */
  detail: string | null;
  /**
   * `profile` is this profile's own token, which no other profile can see. `shared` is the
   * one every profile falls back to, and the only one the command line writes.
   */
  token: TokenSource;
}

export type PrState = 'open' | 'draft' | 'merged' | 'closed';

/** A pull request on GitHub, or a merge request on GitLab. */
export interface PullRequest {
  number: number;
  title: string;
  state: PrState;
  author: string;
  sourceBranch: string;
  targetBranch: string;
  webUrl: string;
  updatedAt: string;
}

export function hostingStatus(path: string): Promise<HostView> {
  return invoke<HostView>('hosting_status', { path });
}

export function hostingLogin(path: string, tokenValue: string): Promise<HostView> {
  return invoke<HostView>('hosting_login', { path, tokenValue });
}

export function hostingLogout(path: string): Promise<HostView> {
  return invoke<HostView>('hosting_logout', { path });
}

/** Opens a pull or merge request on the host, and answers with the one it made. */
export function hostingCreate(
  path: string,
  title: string,
  body: string,
  source: string,
  target: string,
  draft: boolean,
): Promise<PullRequest> {
  return invoke<PullRequest>('hosting_create', { path, title, body, source, target, draft });
}

export function hostingPullRequests(path: string): Promise<PullRequest[]> {
  return invoke<PullRequest[]>('hosting_pull_requests', { path });
}

/**
 * Opens a link in the user's browser.
 *
 * The webview must not navigate there itself: a pull request page would replace the whole
 * window, and there is no way back from it.
 */
export function openInBrowser(url: string): Promise<void> {
  return openUrl(url);
}

/** The todo list an interactive rebase onto `onto` would start from, oldest first. */
export function rebaseTodo(path: string, onto: string): Promise<Todo> {
  return invoke<Todo>('rebase_todo', { path, onto });
}

/** Runs an interactive rebase against a todo the user has decided. */
export function rebaseStart(path: string, onto: string, todo: Todo): Promise<ActionOutcome> {
  return invoke<ActionOutcome>('rebase_start', { path, onto, todo });
}

/* The tab session. Every one of these answers with the whole session, so the caller replaces
   what it holds rather than trying to apply a change to it. */
export const session = {
  get: (): Promise<Session> => invoke<Session>('session_get'),
  open: (path: string): Promise<Session> => invoke<Session>('tab_open', { path }),
  close: (id: number): Promise<Session> => invoke<Session>('tab_close', { id }),
  activate: (id: number): Promise<Session> => invoke<Session>('tab_activate', { id }),
  group: (name: string, ids: number[]): Promise<Session> =>
    invoke<Session>('tab_group', { name, ids }),
  ungroup: (id: number): Promise<Session> => invoke<Session>('tab_ungroup', { id }),
  /** Moves a tab into a group, out of one, or in front of another. */
  move: (id: number, group: number | null, before: number | null): Promise<Session> =>
    invoke<Session>('tab_move', { id, group, before }),
  collapse: (id: number, collapsed: boolean): Promise<Session> =>
    invoke<Session>('group_collapse', { id, collapsed }),
  /** Shows a submodule inside the tab that declares it, rather than in a tab of its own. */
  enterSubmodule: (id: number, path: string): Promise<Session> =>
    invoke<Session>('tab_enter_submodule', { id, path }),
  leaveSubmodule: (id: number): Promise<Session> =>
    invoke<Session>('tab_leave_submodule', { id }),
  /** Sets the picture on a tab; null puts it back to the window's default. */
  icon: (id: number, icon: TabIcon | null): Promise<Session> =>
    invoke<Session>('tab_icon', { id, icon }),
  rename: (id: number, name: string): Promise<Session> =>
    invoke<Session>('group_rename', { id, name }),
  recolour: (id: number, colour: GroupColour): Promise<Session> =>
    invoke<Session>('group_recolour', { id, colour }),
  /** Dissolves a group, leaving its tabs open. */
  dissolve: (id: number): Promise<Session> => invoke<Session>('group_dissolve', { id }),
  closeGroup: (id: number): Promise<Session> => invoke<Session>('group_close', { id }),
};

/* Remotes. Editing one changes configuration rather than a ref, so none of it goes through the
   undo journal, and every edit answers with the whole list. */
export type RemoteEdit =
  | { kind: 'add'; name: string; url: string }
  | { kind: 'remove'; name: string }
  | { kind: 'rename'; name: string; to: string }
  | { kind: 'setUrl'; name: string; url: string }
  | { kind: 'prune'; name: string };

export function remoteList(path: string): Promise<Remote[]> {
  return invoke<Remote[]>('remote_list', { path });
}

export function remoteEdit(path: string, edit: RemoteEdit): Promise<Remote[]> {
  return invoke<Remote[]>('remote_edit', { path, edit });
}

/**
 * Where a commit is served on the web, or null when the remote is not a host we recognise.
 *
 * Null rather than an error: an internal git server has no web address to offer, and that is
 * not worth a dialog.
 */
export function commitUrl(
  path: string,
  oid: string,
  remote: string | null,
): Promise<string | null> {
  return invoke<string | null>('commit_url', { path, oid, remote });
}

/**
 * Asks for a directory, starting in `startIn` where the caller has one to offer.
 *
 * Without it the picker opens wherever the process started — the terminal Coral was launched
 * from, or the home directory — and every one of these questions is about the repository that
 * is open. Writing a patch out of it meant navigating back to it first, every time.
 */
export async function pickDirectory(title: string, startIn?: string): Promise<string | null> {
  const chosen = await openDialog({
    directory: true,
    multiple: false,
    title,
    ...(startIn === undefined ? {} : { defaultPath: startIn }),
  });
  return typeof chosen === 'string' ? chosen : null;
}

/**
 * Patch files to apply, in the order the dialog returns them.
 *
 * A series is several files that must land oldest first, and git names them `0001-`, `0002-`
 * and so on for exactly that reason, so the list is sorted rather than taken as it comes: a
 * file dialog's order is its own business and a series applied out of order fails on the
 * second patch.
 */
export async function pickPatchFiles(title: string, startIn?: string): Promise<string[]> {
  const chosen = await openDialog({
    multiple: true,
    title,
    filters: [{ name: 'Patches', extensions: ['patch', 'diff', 'eml', 'mbox', 'txt'] }],
    ...(startIn === undefined ? {} : { defaultPath: startIn }),
  });
  const files = Array.isArray(chosen) ? chosen : typeof chosen === 'string' ? [chosen] : [];
  return [...files].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
}

/** The worktree, and the two things that change it. */
export function repoStatus(path: string): Promise<Status> {
  return invoke<Status>('repo_status', { path });
}

export function stagePaths(path: string, paths: string[], stage: boolean): Promise<Status> {
  return invoke<Status>('stage_paths', { path, paths, stage });
}

/**
 * Throws away working-tree changes.
 *
 * Two lists, because the two are not the same act: `restore` goes back to what HEAD holds and
 * could be recovered from the object database if it ever came to it, while `remove` is deleted
 * from disk and has never existed anywhere else. The caller decides which path goes in which,
 * so that nothing is deleted the user was not asked about by name.
 */
export function discardPaths(
  path: string,
  restore: string[],
  remove: string[],
): Promise<Status> {
  return invoke<Status>('discard_paths', { path, restore, remove });
}

export function commitStaged(path: string, message: string, amend: boolean): Promise<Status> {
  return invoke<Status>('commit_staged', { path, message, amend });
}

/**
 * How a revision's files are listed.
 *
 * Mirrors `commit::Listing` in Rust. A stash made with `-u` keeps its untracked files in a
 * third parent that a diff against the first never reaches, and only `git stash show` reads
 * all three. It takes any merge for a stash and answers nonsense for one, so the window says
 * which it is rather than letting the engine guess.
 */
export type Listing = 'commit' | 'stash';

/** Everything the detail panel shows for one commit. */
export function commitDetail(
  path: string,
  rev: string,
  listing: Listing = 'commit',
): Promise<CommitDetail> {
  return invoke<CommitDetail>('commit_detail', { path, rev, listing });
}

/* Commit signing. A key belongs to a repository, not to a person: the app-level settings are
   the default and a repository overrides them, which is how git's own config is layered. */
export function signingRead(path: string): Promise<SigningScopes> {
  return invoke<SigningScopes>('signing_read', { path });
}

export function signingSetApp(path: string, config: SigningConfig): Promise<SigningScopes> {
  return invoke<SigningScopes>('signing_set_app', { path, config });
}

export function signingSetRepo(
  path: string,
  overrides: SigningOverrides,
): Promise<SigningScopes> {
  return invoke<SigningScopes>('signing_set_repo', { path, overrides });
}

export function signingKeys(format: SigningFormat, program: string): Promise<SigningKey[]> {
  return invoke<SigningKey[]>('signing_keys', { format, program });
}

export function signingGenerate(
  path: string,
  program: string,
  passphrase: string,
): Promise<SigningKey> {
  return invoke<SigningKey>('signing_generate', { path, program, passphrase });
}

/* How git reaches an ssh server, layered the same way signing is: an app-level default that
   every repository inherits, and a per-repository override. */
export function sshRead(path: string): Promise<SshScopes> {
  return invoke<SshScopes>('ssh_read', { path });
}

export function sshSetApp(path: string, config: SshConfig): Promise<SshScopes> {
  return invoke<SshScopes>('ssh_set_app', { path, config });
}

export function sshSetRepo(path: string, overrides: SshOverrides): Promise<SshScopes> {
  return invoke<SshScopes>('ssh_set_repo', { path, overrides });
}

export function sshKeys(): Promise<SshKey[]> {
  return invoke<SshKey[]>('ssh_keys');
}

export function sshGenerate(
  name: string,
  comment: string,
  passphrase: string,
): Promise<SshKey> {
  return invoke<SshKey>('ssh_generate', { name, comment, passphrase });
}

/** A public key's own text, which is what a host asks to be pasted in. */
export function sshPublicKey(path: string): Promise<string> {
  return invoke<string>('ssh_public_key', { path });
}

/** What this build of Coral is: the version, the commit behind it, and whether it is a debug build. */
export interface AppVersion {
  number: string;
  commit: string | null;
  debug: boolean;
}

/** How many patch files a range would write, asked before anything is written. */
export function patchRangeSize(path: string, from: string, to: string): Promise<number> {
  return invoke<number>('patch_range_size', { path, from, to });
}

export function appVersion(): Promise<AppVersion> {
  return invoke<AppVersion>('app_version');
}
