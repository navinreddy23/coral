import { invoke } from './invoke';
import type { GrepQuery, GrepResult } from './types';

/**
 * The lines of the files at `rev` that match, up to the engine's limit.
 *
 * A newer search of the same repository stops this one, which then rejects as `cancelled`.
 */
export function grepCode(path: string, rev: string, query: GrepQuery): Promise<GrepResult> {
  return invoke<GrepResult>('grep_code', { path, rev, query });
}

/** Stops the code search running in `path`, if there is one. */
export function stopGrep(path: string): Promise<boolean> {
  return invoke<boolean>('stop_grep', { path });
}
