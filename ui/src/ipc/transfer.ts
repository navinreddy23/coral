import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { invoke, isPreview } from './invoke';

/**
 * A network operation the user can watch and stop.
 *
 * Hand-written rather than generated, as everything `coral-app` owns is. Fetch, push and clone
 * are the only things bounded by somebody else's server, and the only ones with no timeout —
 * which is safe precisely because they can be stopped.
 */
export type TransferState = 'running' | 'finished' | 'failed' | 'cancelled';

export interface TransferReport {
  /** What to cancel by. The repository's path, or where a clone will land. */
  key: string;
  /** "Clone", "Fetch", "Push". */
  label: string;
  state: TransferState;
  /** Empty until git says something, which for a host that never answers is never. */
  phase: string;
  current: number;
  total: number;
  percent: number;
  /** The work is happening on the server rather than here. */
  remote: boolean;
  detail: string;
}

/** Stops one, answering whether there was one to stop. */
export function cancelTransfer(key: string): Promise<boolean> {
  return invoke<boolean>('cancel_transfer', { key });
}

/** What is running right now, so a reloaded window finds its way back to a transfer. */
export function runningTransfers(): Promise<string[]> {
  return invoke<string[]>('running_transfers');
}

/**
 * Subscribes to transfer reports.
 *
 * In the browser preview nothing reaches the network, so this subscribes to nothing and hands
 * back a no-op.
 */
export async function onTransfer(
  handle: (report: TransferReport) => void,
): Promise<UnlistenFn> {
  if (isPreview()) return () => undefined;
  return listen<TransferReport>('coral://transfer', (event) => handle(event.payload));
}
