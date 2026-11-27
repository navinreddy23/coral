/**
 * Keyboard shortcuts, taken from the GitKraken client cheat sheet rather than invented, so
 * someone who already uses it does not have to relearn anything. One is added where the sheet
 * has nothing: Ctrl+comma for preferences.
 *
 * Every binding in the sheet is listed, including ones whose action does not exist yet. The
 * help overlay shows the whole table and marks what is live, which is more useful than a short
 * list that silently omits the rest.
 */

export type Context = 'global' | 'message' | 'anywhere';

export interface Binding {
  id: string;
  label: string;
  /** How the sheet writes it, for the help overlay. */
  keys: string;
  group: 'Repo actions' | 'Navigation' | 'Command palette' | 'UI';
  /**
   * Where it applies. `message` bindings fire only inside the commit message box, `anywhere`
   * ones in a text field as well, for a key no field types with.
   */
  context: Context;
  match: (e: KeyEvent) => boolean;
}

/** The parts of a keyboard event a binding looks at. */
export interface KeyEvent {
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
}

/**
 * How a binding is written, by id, or nothing where there is none.
 *
 * The toolbar's buttons are icons now, so their tooltips carry the shortcut as well as the
 * name — and reading it from here rather than spelling it out beside the button keeps one
 * table for the help overlay and the tooltips both.
 */
export function keysOf(id: string): string | null {
  return BINDINGS.find((binding) => binding.id === id)?.keys ?? null;
}

/** True when the primary modifier is held, and no other. Ctrl on Linux and Windows. */
/**
 * A browser key event as the bindings read it.
 *
 * Shift turns Tab into ISO_Left_Tab, whose key WebKitGTK reports as "Unidentified", so the key
 * the event was typed on stands in for it there: without it Ctrl+Shift+Tab did nothing on Linux.
 */
export function keyEventOf(event: {
  key: string;
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}): KeyEvent {
  return {
    key: event.key === 'Unidentified' && event.code === 'Tab' ? 'Tab' : event.key,
    ctrl: event.ctrlKey,
    shift: event.shiftKey,
    alt: event.altKey,
    meta: event.metaKey,
  };
}

function primary(e: KeyEvent): boolean {
  return (e.ctrl || e.meta) && !e.alt;
}

function plain(e: KeyEvent): boolean {
  return !e.ctrl && !e.meta && !e.alt && !e.shift;
}

function is(e: KeyEvent, key: string): boolean {
  return e.key.toLowerCase() === key.toLowerCase();
}

export const BINDINGS: Binding[] = [
  // Repo actions
  { id: 'branch.create', label: 'Create branch', keys: 'Ctrl B', group: 'Repo actions',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'b') },
  { id: 'fetch.all', label: 'Fetch all', keys: 'Ctrl L', group: 'Repo actions',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'l') },
  { id: 'commit', label: 'Commit', keys: 'Ctrl Enter', group: 'Repo actions',
    context: 'message', match: (e) => primary(e) && !e.shift && is(e, 'Enter') },
  { id: 'commit.stageAll', label: 'Stage all and commit', keys: 'Ctrl Shift Enter',
    group: 'Repo actions', context: 'message',
    match: (e) => primary(e) && e.shift && is(e, 'Enter') },
  { id: 'commit.focus', label: 'Focus commit message', keys: 'Ctrl Shift M',
    group: 'Repo actions', context: 'global',
    match: (e) => primary(e) && e.shift && is(e, 'm') },
  { id: 'stage.file', label: 'Stage current file', keys: 'S', group: 'Repo actions',
    context: 'global', match: (e) => plain(e) && is(e, 's') },
  { id: 'unstage.file', label: 'Unstage current file', keys: 'U', group: 'Repo actions',
    context: 'global', match: (e) => plain(e) && is(e, 'u') },
  { id: 'stage.all', label: 'Stage all files', keys: 'Ctrl Shift S', group: 'Repo actions',
    context: 'global', match: (e) => primary(e) && e.shift && is(e, 's') },
  { id: 'unstage.all', label: 'Unstage all files', keys: 'Ctrl Shift U', group: 'Repo actions',
    context: 'global', match: (e) => primary(e) && e.shift && is(e, 'u') },

  // Navigation
  { id: 'select.next', label: 'Select next item', keys: '↓ or J', group: 'Navigation',
    context: 'global', match: (e) => plain(e) && (is(e, 'ArrowDown') || is(e, 'j')) },
  { id: 'select.previous', label: 'Select previous item', keys: '↑ or K', group: 'Navigation',
    context: 'global', match: (e) => plain(e) && (is(e, 'ArrowUp') || is(e, 'k')) },
  { id: 'select.first', label: 'Select first item', keys: 'Ctrl ↑ or Home', group: 'Navigation',
    context: 'global',
    match: (e) => (primary(e) && is(e, 'ArrowUp')) || (plain(e) && is(e, 'Home')) },
  { id: 'select.last', label: 'Select last item', keys: 'Ctrl ↓ or End', group: 'Navigation',
    context: 'global',
    match: (e) => (primary(e) && is(e, 'ArrowDown')) || (plain(e) && is(e, 'End')) },
  // The two every desktop gives the context menu, so a commit's actions are not mouse-only.
  { id: 'select.menu', label: "Open the selected item's menu", keys: 'Menu or Shift F10',
    group: 'Navigation', context: 'global',
    match: (e) =>
      (plain(e) && e.key === 'ContextMenu') ||
      (e.shift && !e.ctrl && !e.meta && !e.alt && e.key === 'F10') },
  // From the graph only, which the window checks: on a focused control, Enter is the control's.
  { id: 'files.focus', label: "Go into the selected commit's files", keys: 'Enter or →',
    group: 'Navigation', context: 'global',
    match: (e) => plain(e) && (e.key === 'Enter' || e.key === 'ArrowRight') },
  // The key every desktop moves between panes with. Anywhere, because the terminal is one of the
  // panes and takes Tab for its shell.
  { id: 'pane.next', label: 'Next pane', keys: 'F6', group: 'Navigation', context: 'anywhere',
    match: (e) => plain(e) && e.key === 'F6' },
  { id: 'pane.previous', label: 'Previous pane', keys: 'Shift F6', group: 'Navigation',
    context: 'anywhere',
    match: (e) => e.shift && !e.ctrl && !e.meta && !e.alt && e.key === 'F6' },
  { id: 'view.close', label: 'Close the file and go back to the graph', keys: 'Esc',
    group: 'Navigation', context: 'global', match: (e) => plain(e) && e.key === 'Escape' },
  { id: 'undo', label: 'Undo', keys: 'Ctrl Z', group: 'Navigation',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'z') },
  { id: 'redo', label: 'Redo', keys: 'Ctrl Y or Ctrl Shift Z', group: 'Navigation',
    context: 'global',
    match: (e) => primary(e) && ((!e.shift && is(e, 'y')) || (e.shift && is(e, 'z'))) },

  // Command palette
  { id: 'palette', label: 'Toggle command palette', keys: 'Ctrl P', group: 'Command palette',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'p') },
  // The one key every list with a filter binds. Plain, because the field it reaches is a
  // filter and not a search of the history, which is what Ctrl F is.
  { id: 'filter.focus', label: 'Filter the branch list', keys: '/', group: 'Navigation',
    context: 'global', match: (e) => plain(e) && e.key === '/' },
  { id: 'search.commits', label: 'Search commits', keys: 'Ctrl F', group: 'Command palette',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'f') },
  { id: 'search.code', label: 'Search the code', keys: 'Ctrl Shift F', group: 'Command palette',
    context: 'global', match: (e) => primary(e) && e.shift && is(e, 'f') },
  { id: 'repo.open', label: 'Open repository', keys: 'Ctrl Shift O', group: 'Command palette',
    context: 'global', match: (e) => primary(e) && e.shift && is(e, 'o') },

  // UI
  { id: 'tab.new', label: 'Open a new tab', keys: 'Ctrl T', group: 'UI',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 't') },
  { id: 'tab.close', label: 'Close tab', keys: 'Ctrl W', group: 'UI',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'w') },
  { id: 'tab.next', label: 'Next tab', keys: 'Ctrl Tab', group: 'UI',
    context: 'global', match: (e) => e.ctrl && !e.shift && is(e, 'Tab') },
  { id: 'tab.previous', label: 'Previous tab', keys: 'Ctrl Shift Tab', group: 'UI',
    context: 'global', match: (e) => e.ctrl && e.shift && is(e, 'Tab') },
  { id: 'panel.left', label: 'Left panel: open, minimised, gone', keys: 'Ctrl \\', group: 'UI',
    context: 'global', match: (e) => primary(e) && is(e, '\\') },
  { id: 'panel.detail', label: 'Toggle right panel', keys: 'Ctrl K', group: 'UI',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'k') },
  { id: 'toolbar', label: 'Toggle toolbar', keys: 'Ctrl U', group: 'UI',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, 'u') },
  // The backtick, as every editor with a terminal binds it. Not `Ctrl T`, which is a new tab
  // everywhere else and would be the wrong reflex here.
  { id: 'terminal', label: 'Toggle the terminal', keys: 'Ctrl `', group: 'UI',
    context: 'global', match: (e) => primary(e) && is(e, '`') },
  // Not on the GitKraken sheet, but what every desktop application binds to its settings.
  { id: 'preferences', label: 'Open preferences', keys: 'Ctrl ,', group: 'UI',
    context: 'global', match: (e) => primary(e) && !e.shift && is(e, ',') },
  { id: 'help', label: 'Show keyboard shortcuts', keys: 'Ctrl /', group: 'UI',
    context: 'global', match: (e) => primary(e) && is(e, '/') },
  // `+` as well as `=`: it is the same key, and which one arrives depends on the layout and
  // on whether Shift is down.
  { id: 'zoom.in', label: 'Zoom in', keys: 'Ctrl =', group: 'UI',
    context: 'global', match: (e) => primary(e) && (e.key === '=' || e.key === '+') },
  { id: 'zoom.out', label: 'Zoom out', keys: 'Ctrl -', group: 'UI',
    context: 'global', match: (e) => primary(e) && !e.shift && (e.key === '-' || e.key === '_') },
  { id: 'zoom.reset', label: 'Actual size', keys: 'Ctrl 0', group: 'UI',
    context: 'global', match: (e) => primary(e) && !e.shift && e.key === '0' },
];

/** `Ctrl 1` to `Ctrl 9` jump to a tab; they are one binding rather than nine. */
export function tabJump(e: KeyEvent): number | null {
  if (!primary(e) || e.shift) return null;
  const n = Number.parseInt(e.key, 10);
  return Number.isInteger(n) && n >= 1 && n <= 9 ? n : null;
}

/**
 * Bindings whose keys a text field already answers: undoing the typing, and moving the caret a
 * paragraph. Taken by the window, Ctrl+Z in the commit message undid the last commit instead.
 */
const FIELD_OWNS = new Set(['undo', 'redo', 'select.first', 'select.last']);

/**
 * The binding an event fires, or null.
 *
 * Typing in the commit message must not stage files because the message contains an "s", so a
 * plain letter only ever acts outside a text field.
 */
export function resolve(e: KeyEvent, context: Context): Binding | null {
  return (
    BINDINGS.find((b) => (b.context === context || b.context === 'anywhere') && b.match(e)) ??
    (context === 'global'
      ? null
      : BINDINGS.find(
          (b) => b.context === 'global' && primary(e) && !FIELD_OWNS.has(b.id) && b.match(e),
        ) ?? null)
  );
}

/** Whether the event came from somewhere that swallows plain keys. */
export function isTextTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable;
}
