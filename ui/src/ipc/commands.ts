import { invoke } from './invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { GroupColour, Session } from '../state/tabs.svelte';
import type {
  Blocks,
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
 * The row a commit sits on, or `null` when it is outside the graph that is loaded.
 *
 * For a commit no ref names: a detached HEAD has a row worth showing and no label to find it
 * by, and the row lookup belongs where the sorted index already is.
 */
export function graphRowOf(path: string, oid: string): Promise<number | null> {
  return invoke<number | null>('graph_row_of', { path, oid });
}

/** The repository's submodules. Empty for a repository that declares none. */
export function repoSubmodules(path: string): Promise<Submodule[]> {
  return invoke<Submodule[]>('repo_submodules', { path });
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
 * Hunks for one file in one commit, or null when the commit did not touch it.
 *
 * One file at a time: a large merge touches thousands, and their patches together are far
 * more than the panel can show or the webview should hold.
 */
export function fileDiff(path: string, rev: string, file: string): Promise<FileDiff | null> {
  return invoke<FileDiff | null>('file_diff', { path, rev, file });
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
): Promise<FileDiff | null> {
  return invoke<FileDiff | null>('worktree_diff', { path, staged, file });
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
  | { kind: 'push'; remote: string | null; setUpstream: boolean }
  | { kind: 'checkout'; rev: string }
  | { kind: 'branchCreate'; name: string; at: string | null; checkout: boolean }
  | { kind: 'branchDelete'; name: string; force: boolean }
  | { kind: 'merge'; rev: string }
  | { kind: 'rebase'; onto: string }
  | { kind: 'cherryPick'; revs: string[] }
  | { kind: 'revert'; revs: string[] }
  | { kind: 'stashPush'; message: string | null }
  | { kind: 'stashApply'; index: number; pop: boolean }
  | { kind: 'stashDrop'; index: number }
  | { kind: 'tagCreate'; name: string; at: string | null; message: string | null }
  | { kind: 'tagDelete'; name: string }
  | { kind: 'reset'; rev: string; mode: 'soft' | 'mixed' | 'hard' }
  | { kind: 'rewrite'; rev: string; how: RewriteKind; message: string | null }
  | { kind: 'worktreeAdd'; path: string; rev: string; branch: string | null }
  | { kind: 'submoduleInit'; path: string | null; recursive: boolean; remote: boolean }
  | { kind: 'submoduleSetUrl'; path: string; url: string }
  | { kind: 'submoduleRemove'; path: string; force: boolean }
  | { kind: 'patch'; rev: string; directory: string }
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

export interface HostView {
  host: Host | null;
  /** Why no host was identified. Not an error: plain git still works without one. */
  detail: string | null;
  signedIn: boolean;
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

/** Asks where a patch file should be written. */
export async function pickDirectory(title: string): Promise<string | null> {
  const chosen = await openDialog({ directory: true, multiple: false, title });
  return typeof chosen === 'string' ? chosen : null;
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

/** Everything the detail panel shows for one commit. */
export function commitDetail(path: string, rev: string): Promise<CommitDetail> {
  return invoke<CommitDetail>('commit_detail', { path, rev });
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
