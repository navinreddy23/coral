import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import type {
  Blocks,
  ConflictedFile,
  FileDiff,
  GitRef,
  Operation,
  OpOutcome,
  Submodule,
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

/** Every ref, each already resolved to the graph row it labels. */
export function repoRefs(path: string): Promise<PlacedRef[]> {
  return invoke<PlacedRef[]>('repo_refs', { path });
}

/** The repository's submodules. Empty for a repository that declares none. */
export function repoSubmodules(path: string): Promise<Submodule[]> {
  return invoke<Submodule[]>('repo_submodules', { path });
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
  | { kind: 'undo' }
  | { kind: 'redo' };

export interface ActionOutcome {
  what: string;
  /** The operation stopped on conflicts and the worktree needs attention. */
  conflicted: boolean;
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
