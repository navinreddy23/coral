/**
 * A dialog that holds the keys while it is open: it takes the focus when it opens and keeps
 * Tab and Shift+Tab going round its own controls, as a desktop's modal dialog does.
 *
 * Without it the focus stayed on whatever had opened the dialog. Tab walked the toolbar behind
 * the scrim, and a question with no default answer — on purpose, for one that destroys
 * something — left Enter to press the button behind it.
 *
 * The focus goes to the control marked `data-autofocus`, else the first; a dialog that has
 * already put it somewhere of its own, a field to type in, is left alone. Only the dialog
 * opened last holds the keys, and a menu opened over it keeps its own.
 */

const FOCUSABLE =
  'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), ' +
  'textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

/** The dialogs open, the one holding the keys last. */
const open: HTMLElement[] = [];

function controls(dialog: HTMLElement): HTMLElement[] {
  return [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (element) => element.closest('[hidden]') === null,
  );
}

function onKey(event: KeyboardEvent) {
  if (event.key !== 'Tab' || event.ctrlKey || event.altKey || event.metaKey) return;
  const dialog = open.at(-1);
  if (dialog === undefined) return;
  const target = event.target instanceof Element ? event.target : null;
  if (target?.closest('[role="menu"]')) return;
  const all = controls(dialog);
  const first = all[0];
  const last = all[all.length - 1];
  if (first === undefined || last === undefined) {
    event.preventDefault();
    return;
  }
  const inside = target !== null && dialog.contains(target);
  if (!inside) {
    event.preventDefault();
    first.focus();
  } else if (event.shiftKey && target === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && target === last) {
    event.preventDefault();
    first.focus();
  }
}

/** An attachment: `<div role="dialog" {@attach modal}>`. */
export function modal(dialog: HTMLElement): () => void {
  if (open.length === 0) document.addEventListener('keydown', onKey, true);
  open.push(dialog);
  if (!dialog.contains(document.activeElement)) {
    // One with nothing to press, a sheet to read, takes the focus itself.
    if (!dialog.hasAttribute('tabindex')) dialog.tabIndex = -1;
    (dialog.querySelector<HTMLElement>('[data-autofocus]') ?? controls(dialog)[0] ?? dialog).focus();
  }
  return () => {
    open.splice(open.indexOf(dialog), 1);
    if (open.length === 0) document.removeEventListener('keydown', onKey, true);
  };
}
