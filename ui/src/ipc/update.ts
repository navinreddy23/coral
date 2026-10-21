import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { invoke } from './invoke';

/** What a check for updates found. */
export interface UpdateCheck {
  /** This build's version. */
  current: string;
  /** The newest published version. */
  latest: string;
  newer: boolean;
  /** The release page of `latest`. */
  url: string;
  /** Whether `latest` can be installed over this copy in place, rather than downloaded by hand. */
  installable: boolean;
}

/** How far an update's download has got. */
export interface UpdateProgress {
  received: number;
  /** Null when the server did not say how large the download is. */
  total: number | null;
}

/** Reads the newest release's `latest.json`. Only ever when asked to. */
export function checkForUpdate(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>('check_for_update');
}

/** Downloads, verifies and installs the newest release, then restarts into it. */
export function installUpdate(): Promise<void> {
  return invoke<void>('install_update');
}

export function onUpdateProgress(handle: (progress: UpdateProgress) => void): Promise<UnlistenFn> {
  return listen<UpdateProgress>('coral://update-progress', (event) => handle(event.payload));
}
