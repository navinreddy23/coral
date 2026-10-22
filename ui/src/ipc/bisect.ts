import { invoke } from './invoke';
import type { Bisection } from './types';

/** The bisection in progress, or null when there is none. */
export function repoBisection(path: string): Promise<Bisection | null> {
  return invoke<Bisection | null>('repo_bisection', { path });
}
