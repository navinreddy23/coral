// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';

import { modal } from '../src/app/modal';

/** A toolbar button behind, with the focus, and a dialog over it. */
function scene(dialog: string) {
  document.body.innerHTML = `<button id="behind">Pull</button><div id="dialog" role="dialog">${dialog}</div>`;
  (document.getElementById('behind') as HTMLElement).focus();
  return document.getElementById('dialog') as HTMLElement;
}

const tab = (shift = false) => {
  const event = new KeyboardEvent('keydown', { key: 'Tab', shiftKey: shift, bubbles: true, cancelable: true });
  (document.activeElement ?? document.body).dispatchEvent(event);
  return event;
};
const focused = () => document.activeElement?.id;

afterEach(() => {
  document.body.innerHTML = '';
});

describe('a dialog', () => {
  it('takes the focus when it opens, on the control it names, else its first', () => {
    // Left behind it, the focus kept a toolbar button, and Enter on a question with no
    // default answer pressed that button instead.
    const named = scene('<button id="cancel">Cancel</button><button id="go" data-autofocus>Go</button>');
    const detach = modal(named);
    expect(focused()).toBe('go');
    detach();

    const plain = scene('<button id="cancel">Cancel</button><button id="go">Go</button>');
    modal(plain)();
    expect(focused()).toBe('cancel');
  });

  it('leaves the focus where the dialog itself already put it', () => {
    const dialog = scene('<input id="name" /><button id="go">Go</button>');
    (document.getElementById('name') as HTMLElement).focus();
    modal(dialog)();
    expect(focused()).toBe('name');
  });

  it('keeps Tab and Shift+Tab inside, going round', () => {
    const dialog = scene('<button id="a">A</button><button id="b">B</button>');
    const detach = modal(dialog);
    expect(focused()).toBe('a');
    // Between two controls inside, Tab is the web view's own.
    expect(tab().defaultPrevented).toBe(false);
    (document.getElementById('b') as HTMLElement).focus();
    expect(tab().defaultPrevented).toBe(true);
    expect(focused()).toBe('a');
    tab(true);
    expect(focused()).toBe('b');
    detach();
  });

  it('brings a Tab pressed behind it back inside', () => {
    const dialog = scene('<button id="a">A</button>');
    const detach = modal(dialog);
    (document.getElementById('behind') as HTMLElement).focus();
    tab();
    expect(focused()).toBe('a');
    detach();
  });

  it('leaves Tab to the dialog opened over it, and to a menu', () => {
    const under = scene('<button id="a">A</button>');
    const off = modal(under);
    document.body.insertAdjacentHTML('beforeend', '<div id="over" role="dialog"><button id="x">X</button><button id="y">Y</button></div>');
    const offOver = modal(document.getElementById('over') as HTMLElement);
    expect(focused()).toBe('x');
    // The dialog under it would have pulled this Tab back to A.
    (document.getElementById('y') as HTMLElement).focus();
    tab();
    expect(focused()).toBe('x');
    offOver();
    document.body.insertAdjacentHTML('beforeend', '<div role="menu"><button id="m">M</button></div>');
    (document.getElementById('m') as HTMLElement).focus();
    expect(tab().defaultPrevented).toBe(false);
    off();
  });
});
