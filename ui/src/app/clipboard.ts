/**
 * Putting text on the system clipboard from inside the webview.
 *
 * Deliberately not a Tauri plugin. The clipboard is one call and one string; adding a plugin
 * for it would put another crate, another permission and another licence entry on the tree for
 * something the webview can already do.
 */
export async function copyText(text: string): Promise<boolean> {
  // The modern API, which needs a secure context. Tauri's custom protocol usually counts as
  // one, but a webview started from a plain file URL does not, so this can simply be absent.
  try {
    if (typeof navigator !== 'undefined' && navigator.clipboard) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Denied or unavailable. The fallback below needs no permission at all.
  }
  return legacyCopy(text);
}

/**
 * The textarea-and-execCommand trick, which WebKit still honours.
 *
 * Off-screen rather than hidden: an element with `display: none` cannot hold a selection, and
 * without a selection there is nothing for the copy to take.
 */
function legacyCopy(text: string): boolean {
  if (typeof document === 'undefined') return false;
  const box = document.createElement('textarea');
  box.value = text;
  box.setAttribute('readonly', '');
  box.style.position = 'fixed';
  box.style.top = '-1000px';
  box.style.opacity = '0';
  document.body.appendChild(box);
  try {
    box.select();
    return document.execCommand('copy');
  } catch {
    return false;
  } finally {
    box.remove();
  }
}
