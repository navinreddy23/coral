// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';

import { land, rows } from '../src/app/rows';

/**
 * A list built by hand: two folders, the first open with two files in it, the second shut,
 * then a file at the top level. Each row records its clicks, and a folder row opens and shuts
 * itself on a click the way FileTree's does.
 */
function tree() {
  document.body.innerHTML = `
    <div id="list">
      <button id="head">Path</button>
      <ul>
        <li><button id="a" data-row="dir" data-depth="0" aria-expanded="true">a</button>
          <ul>
            <li><div class="row"><button id="a1" data-row="show" data-depth="1">a/1</button><button id="act">+</button></div></li>
            <li><button id="a2" data-row="show" data-depth="1">a/2</button></li>
          </ul>
        </li>
        <li><button id="b" data-row="dir" data-depth="0" aria-expanded="false">b</button></li>
        <li><button id="c" data-row="show" data-depth="0">c</button></li>
        <li><input id="field" /></li>
      </ul>
    </div>`;
  const list = document.getElementById('list') as HTMLElement;
  const clicks: string[] = [];
  for (const row of list.querySelectorAll<HTMLElement>('[data-row]')) {
    row.addEventListener('click', () => {
      clicks.push(row.id);
      if (row.dataset.row === 'dir') {
        row.setAttribute('aria-expanded', String(row.getAttribute('aria-expanded') !== 'true'));
      }
    });
  }
  const detach = rows(list);
  return { list, clicks, detach, el: (id: string) => document.getElementById(id) as HTMLElement };
}

function press(target: HTMLElement, key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  return event;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('the arrow keys in a list of rows', () => {
  it('walk the rows in the order they are drawn, showing each one they land on', () => {
    const { el, clicks } = tree();
    el('a1').focus();
    press(el('a1'), 'ArrowDown');
    expect(document.activeElement?.id).toBe('a2');
    press(el('a2'), 'ArrowDown');
    // A folder is only stood on: clicking it would open or shut it on the way past.
    expect(document.activeElement?.id).toBe('b');
    press(el('b'), 'j');
    expect(document.activeElement?.id).toBe('c');
    press(el('c'), 'k');
    expect(document.activeElement?.id).toBe('b');
    expect(clicks).toEqual(['a2', 'c']);
  });

  it('keep the keys from the window, so the graph does not move underneath', () => {
    const { el } = tree();
    const heard = vi.fn();
    window.addEventListener('keydown', heard);
    el('a1').focus();
    const event = press(el('a1'), 'ArrowDown');
    window.removeEventListener('keydown', heard);
    expect(heard).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  it('stop at either end rather than wrapping round', () => {
    const { el, clicks } = tree();
    el('c').focus();
    press(el('c'), 'ArrowDown');
    expect(document.activeElement?.id).toBe('c');
    el('a').focus();
    press(el('a'), 'ArrowUp');
    expect(document.activeElement?.id).toBe('a');
    expect(clicks).toEqual([]);
  });

  it('go to the first and last rows on Home and End, and on Ctrl with an arrow', () => {
    const { el } = tree();
    el('a2').focus();
    press(el('a2'), 'End');
    expect(document.activeElement?.id).toBe('c');
    press(el('c'), 'Home');
    expect(document.activeElement?.id).toBe('a');
    press(el('a'), 'ArrowDown', { ctrlKey: true });
    expect(document.activeElement?.id).toBe('c');
    press(el('c'), 'ArrowUp', { ctrlKey: true });
    expect(document.activeElement?.id).toBe('a');
  });

  it('start from the row a control beside it belongs to', () => {
    // The + beside a file is in the file's row: an arrow pressed on it moves on from the file.
    const { el } = tree();
    el('act').focus();
    press(el('act'), 'ArrowDown');
    expect(document.activeElement?.id).toBe('a2');
  });

  it('leave a key pressed outside any row to the window', () => {
    const { el } = tree();
    el('head').focus();
    const event = press(el('head'), 'ArrowDown');
    expect(event.defaultPrevented).toBe(false);
    expect(document.activeElement?.id).toBe('head');
  });

  it('leave a text field its own arrows', () => {
    const { el } = tree();
    el('field').focus();
    const event = press(el('field'), 'ArrowUp');
    expect(event.defaultPrevented).toBe(false);
    expect(document.activeElement?.id).toBe('field');
  });

  it('leave a key with Alt or Shift to whoever binds it', () => {
    const { el } = tree();
    el('a1').focus();
    expect(press(el('a1'), 'ArrowDown', { altKey: true }).defaultPrevented).toBe(false);
    expect(press(el('a1'), 'ArrowDown', { shiftKey: true }).defaultPrevented).toBe(false);
    expect(document.activeElement?.id).toBe('a1');
  });
});

describe('the arrow keys on a tree', () => {
  it('open a shut folder on the right arrow, and step into an open one', () => {
    const { el, clicks } = tree();
    el('b').focus();
    press(el('b'), 'ArrowRight');
    expect(el('b').getAttribute('aria-expanded')).toBe('true');
    expect(document.activeElement?.id).toBe('b');
    el('a').focus();
    press(el('a'), 'ArrowRight');
    expect(document.activeElement?.id).toBe('a1');
    expect(clicks).toEqual(['b', 'a1']);
  });

  it('shut an open folder on the left arrow, and go up to it from a row inside', () => {
    const { el, clicks } = tree();
    el('a2').focus();
    press(el('a2'), 'ArrowLeft');
    expect(document.activeElement?.id).toBe('a');
    expect(clicks).toEqual([]);
    press(el('a'), 'ArrowLeft');
    expect(el('a').getAttribute('aria-expanded')).toBe('false');
  });

  it('do nothing on the left arrow at the top level', () => {
    const { el } = tree();
    el('c').focus();
    press(el('c'), 'ArrowLeft');
    expect(document.activeElement?.id).toBe('c');
  });
});

describe('a row clicked with the pointer', () => {
  it('takes the focus, so the next arrow walks this list and not the graph', () => {
    const { el } = tree();
    el('head').focus();
    el('a2').click();
    expect(document.activeElement?.id).toBe('a2');
  });

  it('stops answering once the list is gone', () => {
    const { el, detach } = tree();
    detach();
    el('a1').focus();
    expect(press(el('a1'), 'ArrowDown').defaultPrevented).toBe(false);
  });
});

describe('landing on a row from outside the list', () => {
  it('stands on the file at a place, or the last when the list got shorter', () => {
    const { list, clicks } = tree();
    expect(land(list, 1)?.id).toBe('a1');
    expect(document.activeElement?.id).toBe('a1');
    expect(land(list, 40)?.id).toBe('c');
    expect(clicks).toEqual(['a1', 'c']);
  });

  it('passes over a folder to the file after it, since a folder shows nothing', () => {
    // A file staged out of a tree left the keys on the folder after it, and the file still
    // open beside them was the one that had just gone to the other list.
    const { list, clicks } = tree();
    expect(land(list, 0)?.id).toBe('a1');
    // The place is counted in rows, folders included: the third row is b, a shut folder, so
    // the file after it is c.
    expect(land(list, 3)?.id).toBe('c');
    expect(clicks).toEqual(['a1', 'c']);
  });

  it('lands on nothing in an empty list', () => {
    document.body.innerHTML = '<ul id="empty"></ul>';
    expect(land(document.getElementById('empty') as HTMLElement, 0)).toBeNull();
  });
});
