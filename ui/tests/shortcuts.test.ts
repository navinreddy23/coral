import { describe, expect, it } from 'vitest';

import { BINDINGS, keyEventOf, resolve, tabJump, type KeyEvent } from '../src/state/shortcuts';

function key(k: string, mods: Partial<KeyEvent> = {}): KeyEvent {
  return { key: k, ctrl: false, shift: false, alt: false, meta: false, ...mods };
}

describe('shortcuts', () => {
  it('matches the repo actions from the cheat sheet', () => {
    expect(resolve(key('b', { ctrl: true }), 'global')?.id).toBe('branch.create');
    expect(resolve(key('l', { ctrl: true }), 'global')?.id).toBe('fetch.all');
    expect(resolve(key('s', { ctrl: true, shift: true }), 'global')?.id).toBe('stage.all');
    expect(resolve(key('u', { ctrl: true, shift: true }), 'global')?.id).toBe('unstage.all');
  });

  it('accepts both vim and arrow navigation', () => {
    expect(resolve(key('j'), 'global')?.id).toBe('select.next');
    expect(resolve(key('ArrowDown'), 'global')?.id).toBe('select.next');
    expect(resolve(key('k'), 'global')?.id).toBe('select.previous');
    expect(resolve(key('ArrowUp'), 'global')?.id).toBe('select.previous');
    expect(resolve(key('Home'), 'global')?.id).toBe('select.first');
    expect(resolve(key('End'), 'global')?.id).toBe('select.last');
  });

  it('moves between panes on F6 from anywhere, a text field included', () => {
    expect(resolve(key('F6'), 'global')?.id).toBe('pane.next');
    expect(resolve(key('F6'), 'message')?.id).toBe('pane.next');
    expect(resolve(key('F6', { shift: true }), 'message')?.id).toBe('pane.previous');
  });

  it('goes into the files on Enter or the right arrow, and back out on Escape', () => {
    expect(resolve(key('Enter'), 'global')?.id).toBe('files.focus');
    expect(resolve(key('ArrowRight'), 'global')?.id).toBe('files.focus');
    expect(resolve(key('Escape'), 'global')?.id).toBe('view.close');
    // Typing a message is not leaving anything.
    expect(resolve(key('Enter'), 'message')).toBeNull();
    expect(resolve(key('Escape'), 'message')).toBeNull();
  });

  it('opens the selected row\'s menu from the Menu key and Shift+F10', () => {
    expect(resolve(key('ContextMenu'), 'global')?.id).toBe('select.menu');
    expect(resolve(key('F10', { shift: true }), 'global')?.id).toBe('select.menu');
    expect(resolve(key('F10'), 'global')).toBeNull();
  });

  it('opens Preferences on Ctrl+comma, and not on a plain comma', () => {
    expect(resolve(key(',', { ctrl: true }), 'global')?.id).toBe('preferences');
    expect(resolve(key(','), 'global')).toBeNull();
    expect(resolve(key(','), 'message')).toBeNull();
  });

  it('accepts either spelling of redo', () => {
    expect(resolve(key('y', { ctrl: true }), 'global')?.id).toBe('redo');
    expect(resolve(key('z', { ctrl: true, shift: true }), 'global')?.id).toBe('redo');
    expect(resolve(key('z', { ctrl: true }), 'global')?.id).toBe('undo');
  });

  /* Typing a commit message contains the letter s; staging every file because of it would be
     the single most annoying possible bug. */
  it('does not fire a plain letter inside the message box', () => {
    expect(resolve(key('s'), 'global')?.id).toBe('stage.file');
    expect(resolve(key('s'), 'message')).toBeNull();
    expect(resolve(key('j'), 'message')).toBeNull();
  });

  it('still allows modified shortcuts from inside the message box', () => {
    expect(resolve(key('Enter', { ctrl: true }), 'message')?.id).toBe('commit');
    expect(resolve(key('Enter', { ctrl: true, shift: true }), 'message')?.id).toBe(
      'commit.stageAll',
    );
    expect(resolve(key('t', { ctrl: true }), 'message')?.id).toBe('tab.new');
  });

  /* Ctrl+Z in the message box undid the last commit rather than the last word typed. */
  it('leaves undo, redo and paragraph moves to the text field they are typed in', () => {
    expect(resolve(key('z', { ctrl: true }), 'message')).toBeNull();
    expect(resolve(key('y', { ctrl: true }), 'message')).toBeNull();
    expect(resolve(key('z', { ctrl: true, shift: true }), 'message')).toBeNull();
    expect(resolve(key('ArrowUp', { ctrl: true }), 'message')).toBeNull();
    expect(resolve(key('ArrowDown', { ctrl: true }), 'message')).toBeNull();
    expect(resolve(key('z', { ctrl: true }), 'global')?.id).toBe('undo');
    expect(resolve(key('ArrowDown', { ctrl: true }), 'global')?.id).toBe('select.last');
  });

  it('reads Ctrl 1 to 9 as a tab jump and nothing else', () => {
    expect(tabJump(key('1', { ctrl: true }))).toBe(1);
    expect(tabJump(key('9', { ctrl: true }))).toBe(9);
    expect(tabJump(key('0', { ctrl: true }))).toBeNull();
    expect(tabJump(key('1'))).toBeNull();
    expect(tabJump(key('1', { ctrl: true, shift: true }))).toBeNull();
  });

  it('separates next and previous tab by the shift key alone', () => {
    expect(resolve(key('Tab', { ctrl: true }), 'global')?.id).toBe('tab.next');
    expect(resolve(key('Tab', { ctrl: true, shift: true }), 'global')?.id).toBe('tab.previous');
  });

  // Shift turns Tab into ISO_Left_Tab, which WebKitGTK reports as an "Unidentified" key, so
  // Ctrl+Shift+Tab never reached the previous tab on Linux.
  it('reads the previous-tab keys as WebKitGTK reports them', () => {
    const gtk = { key: 'Unidentified', code: 'Tab', ctrlKey: true, shiftKey: true, altKey: false, metaKey: false };
    expect(resolve(keyEventOf(gtk), 'global')?.id).toBe('tab.previous');
    const other = { ...gtk, key: 'Unidentified', code: 'KeyQ' };
    expect(keyEventOf(other).key).toBe('Unidentified');
  });

  it('has no duplicate ids and no binding without a key description', () => {
    const ids = BINDINGS.map((b) => b.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(BINDINGS.every((b) => b.keys.length > 0 && b.label.length > 0)).toBe(true);
  });

  /* Two bindings that fire on the same event would make one of them unreachable. */
  it('has no two bindings matching the same event in the same context', () => {
    const events = [
      key('s'), key('u'), key('j'), key('k'),
      key('b', { ctrl: true }), key('l', { ctrl: true }), key('p', { ctrl: true }),
      key('f', { ctrl: true }), key('t', { ctrl: true }), key('w', { ctrl: true }),
      key('k', { ctrl: true }), key('u', { ctrl: true }), key('z', { ctrl: true }),
      key('s', { ctrl: true, shift: true }), key('u', { ctrl: true, shift: true }),
      key('Enter', { ctrl: true }),
    ];
    for (const e of events) {
      for (const context of ['global', 'message'] as const) {
        const hits = BINDINGS.filter((b) => b.context === context && b.match(e));
        expect(hits.length, `${e.key} in ${context} matched ${hits.map((h) => h.id)}`)
          .toBeLessThanOrEqual(1);
      }
    }
  });

  it('zooms with Ctrl = (or +), Ctrl - and Ctrl 0, leaving Ctrl 1 to 9 for the tabs', () => {
    expect(resolve(key('=', { ctrl: true }), 'global')?.id).toBe('zoom.in');
    expect(resolve(key('+', { ctrl: true, shift: true }), 'global')?.id).toBe('zoom.in');
    expect(resolve(key('-', { ctrl: true }), 'global')?.id).toBe('zoom.out');
    expect(resolve(key('0', { ctrl: true }), 'global')?.id).toBe('zoom.reset');
    expect(tabJump(key('0', { ctrl: true }))).toBeNull();
    expect(tabJump(key('1', { ctrl: true }))).toBe(1);
    expect(resolve(key('='), 'global')).toBeNull();
  });
});
