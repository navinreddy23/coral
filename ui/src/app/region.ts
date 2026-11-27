/**
 * F6 and Shift+F6 between the window's panes, the key every desktop moves between panes with.
 *
 * Tab cannot do it here: the terminal's shell takes Tab for completion, as it must, so a pane
 * reached by Tab could only be left by closing it.
 *
 * A pane is marked `data-panel`; anything outside every pane is the middle, which is the graph
 * or the file open over it. The graph's rows are kept out of the tab order and walked with keys
 * bound on the window, so the graph has the keys when nothing has the focus.
 */

type Region = 'refs' | 'main' | 'files' | 'terminal';

const ORDER: Region[] = ['refs', 'main', 'files', 'terminal'];

function regionOf(element: Element | null): Region {
  const panel = element?.closest<HTMLElement>('[data-panel]')?.dataset.panel;
  return ORDER.find((region) => region === panel) ?? 'main';
}

/**
 * The first match that is on screen. A tab's terminal stays mounted, hidden, while another tab
 * is showing, so the first match in the page can be one nobody can see.
 */
function shown(selector: string): HTMLElement | null {
  for (const element of document.querySelectorAll<HTMLElement>(selector)) {
    if (element.closest('[hidden]') === null) return element;
  }
  return null;
}

function present(region: Region): boolean {
  return region === 'main' || shown(`[data-panel="${region}"]`) !== null;
}

/** Where the focus goes in a pane: what the pane has selected, without opening anything. */
function target(region: Region): HTMLElement | null {
  const panel = `[data-panel="${region}"]`;
  switch (region) {
    case 'refs':
      return shown(`${panel} [data-row].current`) ?? shown(`${panel} [data-row]`);
    case 'files':
      return shown(`${panel} [data-row].open`) ?? shown(`${panel} [data-row]`);
    case 'terminal':
      return shown(`${panel} textarea`);
    case 'main':
      return shown('section.diff .scroll');
  }
}

/**
 * Moves the focus to the next pane, or the previous one. A pane with nothing in it to stand on
 * is passed over, except the middle, where standing on nothing is how the graph gets the keys.
 */
export function cycleRegion(delta: 1 | -1): void {
  const regions = ORDER.filter(present);
  const at = regions.indexOf(regionOf(document.activeElement));
  for (let step = 1; step <= regions.length; step += 1) {
    const next = regions[(at + delta * step + regions.length * step) % regions.length] ?? 'main';
    const element = target(next);
    if (element !== null) {
      element.focus();
      return;
    }
    if (next === 'main') {
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
      return;
    }
  }
}
