/**
 * Hands the focus back, when something opened over the window closes, to whatever had it when
 * it opened, as a desktop does with a menu or a dialog.
 *
 * Called while the overlay is being created, before it moves the focus into itself. Left to
 * the web view, the focus fell on the page when the overlay went: the file the keys had been
 * walking lost them, and the next arrow moved the graph hidden under it.
 */
export function handBackFocus(): void {
  const before = document.activeElement;
  $effect(() => () => {
    if (before instanceof HTMLElement && before !== document.body && before.isConnected) {
      before.focus({ preventScroll: true });
    } else if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  });
}
