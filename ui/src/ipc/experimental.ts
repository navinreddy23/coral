import { invoke } from './invoke';

/**
 * Which git Coral runs.
 *
 * Hand-written rather than generated, as everything `coral-app` owns is: `ts-rs` covers the
 * engine's types in `coral-core`, not the command layer's.
 */
export type GitChoice =
  | { kind: 'system' }
  | { kind: 'bundled' }
  | { kind: 'custom'; path: string };

export interface GitCandidate {
  choice: GitChoice;
  path: string;
  /** What it answers to `--version`, or null when it cannot be run. */
  version: string | null;
  /** Why it cannot be used, when it cannot. */
  problem: string | null;
}

export interface GitView {
  chosen: GitChoice;
  candidates: GitCandidate[];
  /** The git actually in use, which is not always the one chosen. */
  inUse: string;
  inUseVersion: string | null;
}

export function experimentalGit(): Promise<GitView> {
  return invoke<GitView>('experimental_git');
}

export function experimentalSetGit(choice: GitChoice): Promise<GitView> {
  return invoke<GitView>('experimental_set_git', { choice });
}
