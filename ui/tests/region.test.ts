// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';

import { cycleRegion } from '../src/app/region';

/** A window with every pane: the branch list, a file open in the middle, the files, a terminal. */
function window_(withDiff = true, withTerminal = true) {
  document.body.innerHTML = `
    <aside data-panel="refs">
      <button id="other" data-row="show">other</button>
      <button id="main" class="current" data-row="show">main</button>
    </aside>
    <main>
      ${withDiff ? '<section class="diff"><div id="scroll" class="scroll" tabindex="0"></div></section>' : ''}
      <button id="pill">a pill in the graph</button>
    </main>
    <aside data-panel="files">
      <button id="one" data-row="show">one</button>
      <button id="two" class="open" data-row="show">two</button>
    </aside>
    ${withTerminal ? '<section data-panel="terminal"><textarea id="shell"></textarea></section>' : ''}`;
}

const focused = () => document.activeElement?.id ?? document.activeElement?.tagName;

afterEach(() => {
  document.body.innerHTML = '';
});

describe('moving between the panes', () => {
  it('goes round them in the order they are laid out', () => {
    window_();
    (document.getElementById('main') as HTMLElement).focus();
    cycleRegion(1);
    expect(focused()).toBe('scroll');
    cycleRegion(1);
    // The open file rather than the first, and without opening anything.
    expect(focused()).toBe('two');
    cycleRegion(1);
    expect(focused()).toBe('shell');
    cycleRegion(1);
    // The checked-out branch.
    expect(focused()).toBe('main');
  });

  it('goes back the other way', () => {
    window_();
    (document.getElementById('shell') as HTMLElement).focus();
    cycleRegion(-1);
    expect(focused()).toBe('two');
    cycleRegion(-1);
    expect(focused()).toBe('scroll');
  });

  it('hands the graph the keys by focusing nothing, when no file is open over it', () => {
    window_(false, false);
    (document.getElementById('two') as HTMLElement).focus();
    cycleRegion(-1);
    expect(document.activeElement).toBe(document.body);
    cycleRegion(-1);
    expect(focused()).toBe('main');
  });

  it('counts a control in the graph as the graph', () => {
    window_(false);
    (document.getElementById('pill') as HTMLElement).focus();
    cycleRegion(1);
    expect(focused()).toBe('two');
  });

  it('leaves out a pane that is not there', () => {
    window_(true, false);
    (document.getElementById('two') as HTMLElement).focus();
    cycleRegion(1);
    expect(focused()).toBe('main');
  });
});

describe('a pane with nothing to stand on', () => {
  it('is passed over rather than trapping the keys in the middle', () => {
    window_(false);
    for (const row of document.querySelectorAll('[data-panel="files"] [data-row]')) row.remove();
    (document.getElementById('main') as HTMLElement).focus();
    cycleRegion(1);
    expect(document.activeElement).toBe(document.body);
    cycleRegion(1);
    expect(focused()).toBe('shell');
  });
});

describe('a terminal kept for another tab', () => {
  it('is not where the keys go', () => {
    window_(false);
    document.body.insertAdjacentHTML(
      'afterbegin',
      '<div hidden><section data-panel="terminal"><textarea id="elsewhere"></textarea></section></div>',
    );
    (document.getElementById('two') as HTMLElement).focus();
    cycleRegion(1);
    expect(focused()).toBe('shell');
  });
});
