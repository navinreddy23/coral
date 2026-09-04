import { invoke as tauriInvoke } from '@tauri-apps/api/core';

/**
 * Calls the engine.
 *
 * Everything in `ipc/` goes through here rather than through Tauri directly, so the window can
 * be opened in an ordinary browser during development and answered with fixtures. That is the
 * only way to look at the interface with devtools, or to photograph it at all: WebKitGTK
 * renders in a separate process that no screen capture on this platform can read.
 *
 * The fixtures are reached by a dynamic import inside a branch Vite compiles away, not a
 * static one. A static import kept the whole module in the release bundle — its top-level
 * initialiser is not something the bundler will prove pure — so a build shipped with invented
 * commit messages and author names inside it. Checked by `just check`.
 */
export async function invoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (import.meta.env.DEV && isPreview()) {
    const { preview } = await import('./preview');
    return preview(command, args) as T;
  }
  return tauriInvoke<T>(command, args);
}

/** True when the window is running in a plain browser rather than in the Tauri shell. */
function isPreview(): boolean {
  return typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);
}
