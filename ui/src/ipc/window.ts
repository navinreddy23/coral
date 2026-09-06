import { getCurrentWindow, type Window } from '@tauri-apps/api/window';

/**
 * Which edge or corner a resize is pulling.
 *
 * Spelled out here because `@tauri-apps/api` declares the type but does not export it, and the
 * strings are the wire format the window manager is given.
 */
export type ResizeDirection =
  | 'North' | 'NorthEast' | 'East' | 'SouthEast'
  | 'South' | 'SouthWest' | 'West' | 'NorthWest';

/**
 * The window itself, as opposed to anything shown in it.
 *
 * Coral draws its own title bar, so the buttons that close and resize the window are ordinary
 * page elements and have to reach the window manager through here. Every call is resolved
 * lazily and swallowed on failure: the same page runs under vitest and in a plain browser,
 * where there is no window to act on and a throwing import would take the whole module graph
 * with it.
 */
function current(): Window | null {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

export async function minimize(): Promise<void> {
  await current()?.minimize();
}

export async function toggleMaximize(): Promise<void> {
  await current()?.toggleMaximize();
}

export async function close(): Promise<void> {
  await current()?.close();
}

export async function isMaximized(): Promise<boolean> {
  return (await current()?.isMaximized()) ?? false;
}

/**
 * Hands the drag to the window manager, which then owns the pointer until the button comes up.
 *
 * This is how an edge is resized once the decorations are gone. Doing it in the page instead —
 * following the pointer and setting a size per frame — resizes a webview several times a second
 * and is visibly behind the cursor the whole way.
 */
export async function startResize(direction: ResizeDirection): Promise<void> {
  await current()?.startResizeDragging(direction);
}

/**
 * Whether the desktop is drawing this window's border.
 *
 * Asked rather than assumed, so the page does not have to know which platform it is on: where
 * the native title bar is the right answer the configuration keeps it, and the answer here is
 * what decides whether Coral draws a title bar of its own.
 */
export async function isDecorated(): Promise<boolean> {
  return (await current()?.isDecorated()) ?? true;
}

/** Gives the window back to the desktop's own title bar, or takes it away again. */
export async function setDecorations(on: boolean): Promise<void> {
  await current()?.setDecorations(on);
}

/** Calls back whenever the window is resized, which is the only notice a maximise gives. */
export async function onResized(run: () => void): Promise<() => void> {
  const window = current();
  if (!window) return () => undefined;
  try {
    return await window.onResized(() => run());
  } catch {
    return () => undefined;
  }
}
