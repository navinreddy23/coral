/**
 * The arrow keys for a list of rows: the list that has the focus walks its own rows.
 *
 * The window binds the arrows to the commit graph, and a file list's rows are buttons with no
 * key handling of their own, so an arrow pressed on a file that had just been clicked moved
 * the selected commit instead of the file. A list attached here takes its arrows before the
 * window hears them, as the Splitter does with the arrows that move it.
 *
 * Rows are the `[data-row]` elements inside the list, in the order they are drawn:
 *
 * - `data-row="show"` is shown on arrival — clicked, so the diff or the commit follows the keys
 *   the way the graph's panel follows its selection;
 * - `data-row="dir"` is a folder with `aria-expanded`, only stood on, since clicking it on the
 *   way past would open or shut it;
 * - any other value is only stood on.
 *
 * `data-depth` places a row in a tree, for ← and →, which follow the tree view pattern:
 * https://www.w3.org/WAI/ARIA/apg/patterns/treeview/
 */

const FIELDS = new Set(['INPUT', 'TEXTAREA', 'SELECT']);

function isField(target: Element): boolean {
  return FIELDS.has(target.tagName) || (target instanceof HTMLElement && target.isContentEditable);
}

function rowsOf(list: HTMLElement): HTMLElement[] {
  return [...list.querySelectorAll<HTMLElement>('[data-row]')].filter(
    (row) => !(row instanceof HTMLButtonElement && row.disabled),
  );
}

/** The row a key was pressed in: the row itself, or the row a control beside it belongs to. */
function rowAt(target: Element, list: HTMLElement): HTMLElement | null {
  const own = target.closest<HTMLElement>('[data-row]');
  if (own !== null && list.contains(own)) return own;
  const row = target.closest('.row, li')?.querySelector<HTMLElement>('[data-row]') ?? null;
  return row !== null && list.contains(row) ? row : null;
}

const depthOf = (row: HTMLElement) => Number(row.dataset.depth ?? 0);
const isDir = (row: HTMLElement) => row.dataset.row === 'dir';
const isOpen = (row: HTMLElement) => row.getAttribute('aria-expanded') === 'true';

function arrive(row: HTMLElement) {
  row.focus();
  if (row.dataset.row === 'show') row.click();
}

/** The folder a row is in, which is the nearest folder above it one level up. */
function folderOf(all: HTMLElement[], at: number): HTMLElement | null {
  const depth = depthOf(all[at] as HTMLElement);
  for (let i = at - 1; i >= 0; i -= 1) {
    const row = all[i] as HTMLElement;
    if (depthOf(row) < depth) return isDir(row) ? row : null;
  }
  return null;
}

/**
 * Where a key goes from a row: another row, `'toggle'` for a folder to open or shut, or
 * `undefined` for a key that is not the list's.
 */
function destination(all: HTMLElement[], at: number, key: string, ctrl: boolean) {
  const row = all[at] as HTMLElement;
  const first = all[0] as HTMLElement;
  const last = all[all.length - 1] as HTMLElement;
  if (ctrl) {
    if (key === 'ArrowUp') return first;
    if (key === 'ArrowDown') return last;
    return undefined;
  }
  switch (key) {
    case 'ArrowDown':
    case 'j':
      return all[at + 1] ?? row;
    case 'ArrowUp':
    case 'k':
      return all[at - 1] ?? row;
    case 'Home':
      return first;
    case 'End':
      return last;
    case 'ArrowRight': {
      if (!isDir(row)) return row;
      if (!isOpen(row)) return 'toggle';
      const child = all[at + 1];
      return child !== undefined && depthOf(child) > depthOf(row) ? child : row;
    }
    case 'ArrowLeft':
      if (isDir(row) && isOpen(row)) return 'toggle';
      return folderOf(all, at) ?? row;
    default:
      return undefined;
  }
}

function onKey(list: HTMLElement, event: KeyboardEvent) {
  if (event.altKey || event.shiftKey || event.metaKey) return;
  const target = event.target;
  if (!(target instanceof Element) || isField(target)) return;
  const row = rowAt(target, list);
  if (row === null) return;
  const all = rowsOf(list);
  const at = all.indexOf(row);
  if (at < 0) return;
  const next = destination(all, at, event.key, event.ctrlKey);
  if (next === undefined) return;
  event.preventDefault();
  event.stopPropagation();
  if (next === 'toggle') row.click();
  else if (next !== row || document.activeElement !== row) arrive(next);
}

/**
 * Focuses a row the pointer clicked.
 *
 * WebKit's rule for whether a click focuses a button is not Chrome's, and a row left without
 * the focus leaves the arrows with the graph.
 */
function onClick(list: HTMLElement, event: MouseEvent) {
  const row = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-row]') : null;
  if (row !== null && list.contains(row) && document.activeElement !== row) row.focus();
}

/** An attachment: `<ul {@attach rows}>`. */
export function rows(list: HTMLElement): () => void {
  const key = (event: KeyboardEvent) => onKey(list, event);
  const click = (event: MouseEvent) => onClick(list, event);
  list.addEventListener('keydown', key);
  list.addEventListener('click', click);
  return () => {
    list.removeEventListener('keydown', key);
    list.removeEventListener('click', click);
  };
}

/**
 * Stands on the first file at or after a place in a list, or the last file before it, and
 * shows it. For the keys that act on a row and take it away, so the focus stays in the list,
 * and for going into a list from outside. A folder is passed over: standing on one shows
 * nothing, and the file left open beside it would be the one that had just gone.
 */
export function land(list: HTMLElement, index: number): HTMLElement | null {
  const all = rowsOf(list);
  const shown = (row: HTMLElement | undefined) => row?.dataset.row === 'show';
  const at = Math.max(index, 0);
  const row = all.slice(at).find(shown) ?? all.slice(0, at).reverse().find(shown);
  if (row === undefined) return null;
  arrive(row);
  return row;
}

/** Where a row is in its list, for `land` once the row has gone. */
export function placeOf(list: HTMLElement, row: Element): number {
  return rowsOf(list).indexOf(row as HTMLElement);
}
