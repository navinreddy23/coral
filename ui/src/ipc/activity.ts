import { invoke } from '@tauri-apps/api/core';

/** How loud an entry is. Written by `coral-app`'s `activity` module. */
export type ActivityLevel = 'info' | 'warn' | 'error';

/**
 * One line of the log.
 *
 * Hand-written rather than generated: `coral-app`'s own types do not go through `ts-rs`, which
 * only covers the engine types in `coral-core`.
 */
export interface ActivityEntry {
  /** Milliseconds since the Unix epoch, so the window formats it in the user's locale. */
  at: number;
  level: ActivityLevel;
  /** The repository it belongs to, or null for an application entry. */
  repo: string | null;
  message: string;
  /** How long the operation took, on the line that ends one. */
  millis: number | null;
}

/** The log for one repository, or the application's own when `repo` is null. */
export function activityLog(repo: string | null): Promise<ActivityEntry[]> {
  return invoke<ActivityEntry[]>('activity_log', { repo });
}

/** Empties one channel. */
export function activityClear(repo: string | null): Promise<void> {
  return invoke<void>('activity_clear', { repo });
}
