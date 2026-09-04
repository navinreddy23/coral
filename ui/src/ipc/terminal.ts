import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { invoke, isPreview } from './invoke';

/** A shell that has been started. */
export interface Opened {
  id: number;
  shell: string;
}

/**
 * Starts a shell in a repository.
 *
 * The size is given up front: a shell asks the terminal how wide it is before it draws its
 * first prompt, and one started at the wrong size wraps every line until something resizes it.
 */
export function terminalOpen(path: string, cols: number, rows: number): Promise<Opened> {
  return invoke<Opened>('terminal_open', { path, cols, rows });
}

/** Sends keystrokes. */
export function terminalWrite(id: number, data: string): Promise<void> {
  return invoke<void>('terminal_write', { id, data });
}

/** Tells the shell the pane changed size. */
export function terminalResize(id: number, cols: number, rows: number): Promise<void> {
  return invoke<void>('terminal_resize', { id, cols, rows });
}

/** Ends the shell. */
export function terminalClose(id: number): Promise<void> {
  return invoke<void>('terminal_close', { id });
}

/**
 * Subscribes to a terminal's output.
 *
 * One event name per terminal, so two open at once do not have to filter each other's bytes
 * out of a shared stream.
 */
export async function terminalListen(
  id: number,
  onData: (text: string) => void,
  onClosed: () => void,
): Promise<UnlistenFn> {
  // In a browser there is no event channel and no shell behind it; the fixtures play a short
  // session instead, so the pane can be looked at. Compiled away in a release build.
  if (import.meta.env.DEV && isPreview()) {
    const { previewTerminal } = await import('./preview');
    return previewTerminal(onData);
  }
  const data = await listen<string>(`terminal://${id}`, (e) => onData(e.payload));
  const closed = await listen(`terminal://${id}/closed`, () => onClosed());
  return () => {
    data();
    closed();
  };
}
