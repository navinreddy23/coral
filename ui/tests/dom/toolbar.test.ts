// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import Toolbar from '../../src/app/Toolbar.svelte';

afterEach(cleanup);

/** The toolbar as it appears in a repository, with nothing running and nothing compared. */
function bar(overrides: Record<string, unknown> = {}) {
  return render(Toolbar, {
    props: {
      repo: 'coral',
      path: '/home/dev/coral',
      submodule: null,
      branch: 'master',
      busy: false,
      comparing: false,
      terminalOpen: false,
      leftPanel: 'open',
      stashes: 0,
      dirty: true,
    journal: { undo: null, redo: null },
      rightPanel: true,
      rightPanelUsable: true,
      onAction: vi.fn(),
      onLeaveSubmodule: vi.fn(),
      onPullMenu: vi.fn(),
      onPushMenu: vi.fn(),
      ...overrides,
    },
  });
}

function buttons(container: HTMLElement): HTMLButtonElement[] {
  return [...container.querySelectorAll('button.action')] as HTMLButtonElement[];
}

describe('the toolbar', () => {
  it('offers every action it used to, in the order it used to', () => {
    // The words are gone from the face of the buttons; nothing else about them is.
    const { container } = bar();
    expect(buttons(container).map((b) => b.getAttribute('aria-label'))).toEqual([
      'Undo', 'Redo', 'Fetch', 'Pull', 'Push', 'Branch', 'Stash', 'Pop', 'Patch',
      'Left panel', 'Right panel', 'Terminal',
    ]);
  });

  it('gives every button a name a screen reader can read', () => {
    // This is now the only name they have, so it is not decoration.
    for (const button of buttons(bar().container)) {
      expect(button.getAttribute('aria-label')?.length ?? 0).toBeGreaterThan(0);
    }
  });

  it('names the action first in the tooltip, then what it does', () => {
    const { container } = bar();
    const fetch = buttons(container).find((b) => b.getAttribute('aria-label') === 'Fetch');
    expect(fetch?.title).toMatch(/^Fetch — /);
  });

  it('carries the keystroke in the tooltip where the action has one', () => {
    // Read from the same table the help overlay is built from, not spelled out twice.
    const { container } = bar();
    const named = (label: string) =>
      buttons(container).find((b) => b.getAttribute('aria-label') === label)?.title ?? '';
    expect(named('Undo')).toContain('(Ctrl Z)');
    expect(named('Branch')).toContain('(Ctrl B)');
    expect(named('Terminal')).toContain('(Ctrl `)');
    // Pull has no binding in the table, so it must not invent one.
    expect(named('Pull')).not.toContain('(');
  });

  it('says which of its two jobs the patch button will do', () => {
    expect(bar().container.querySelector('[aria-label="Patch"]')?.getAttribute('title'))
      .toContain('apply a patch file');
    const comparing = bar({ comparing: true });
    expect(comparing.container.querySelector('[aria-label="Patch"]')?.getAttribute('title'))
      .toContain('write the commits');
  });

  it('draws every action, so none of them is a typeface glyph', () => {
    for (const button of buttons(bar().container)) {
      expect(button.querySelector('svg'), button.getAttribute('aria-label') ?? '').not.toBeNull();
    }
  });

  it('separates the three groups with a rule rather than with space', () => {
    // Space is the first thing a narrow window takes away, and the groups have to survive it.
    expect(bar().container.querySelectorAll('.actions .rule')).toHaveLength(2);
  });

  it('offers a choice beside pull and push, and beside nothing else', () => {
    const { container } = bar();
    const carets = [...container.querySelectorAll('button.caret')].map((c) => c.getAttribute('title'));
    expect(carets).toEqual(['Choose how to pull', 'Choose what to push']);
  });

  it('keeps the parts of the window at the far end, away from the repository actions', () => {
    // They act on the window, not on the repository, and sitting inside the run of git actions
    // is what made that unclear.
    const { container } = bar();
    for (const label of ['Left panel', 'Right panel', 'Terminal']) {
      expect(container.querySelector(`.trailing [aria-label="${label}"]`), label).not.toBeNull();
      expect(container.querySelector(`.actions [aria-label="${label}"]`), label).toBeNull();
    }
  });

  it('says whether the terminal pane is showing', () => {
    expect(bar().container.querySelector('[aria-label="Terminal"]')?.getAttribute('aria-pressed'))
      .toBe('false');
    expect(bar({ terminalOpen: true }).container
      .querySelector('[aria-label="Terminal"]')?.getAttribute('aria-pressed')).toBe('true');
  });

  it('disables the repository actions while one is running, and leaves the terminal alone', () => {
    const { container } = bar({ busy: true });
    const named = (label: string) =>
      buttons(container).find((b) => b.getAttribute('aria-label') === label);
    expect(named('Push')?.disabled).toBe(true);
    expect(named('Terminal')?.disabled).toBe(false);
  });

  it('draws each side panel as showing or as folded away', () => {
    // The glyph carries the state, not a lit button: both panels are showing in the state the
    // window ships in, so lighting them would mean a toolbar that glows for no reason.
    const shown = bar().container;
    const hidden = bar({ leftPanel: 'hidden', rightPanel: false }).container;
    const strips = (root: HTMLElement, label: string) =>
      root.querySelector(`[aria-label="${label}"] svg`)?.querySelectorAll('path[fill]').length ?? 0;

    expect(strips(shown, 'Left panel'), 'showing: a filled strip').toBe(1);
    expect(strips(hidden, 'Left panel'), 'folded: a ruled edge, nothing filled').toBe(0);
    expect(strips(shown, 'Right panel')).toBe(1);
    expect(strips(hidden, 'Right panel')).toBe(0);
  });

  it('draws the left panel minimised as its own thing, not as either of the other two', () => {
    // Three states behind one button, so the glyph is the only thing saying which one it is at.
    const drawn = (state: string) => {
      const svg = bar({ leftPanel: state }).container.querySelector('[aria-label="Left panel"] svg');
      return svg?.innerHTML ?? '';
    };
    const [open, rail, hidden] = [drawn('open'), drawn('rail'), drawn('hidden')];
    expect(new Set([open, rail, hidden]).size, 'three states, three glyphs').toBe(3);

    const dots = (state: string) =>
      bar({ leftPanel: state }).container.querySelectorAll('[aria-label="Left panel"] svg circle').length;
    // Minimised is the panel with its section icons still in it, which is what the rail is. A
    // thinner version of the open panel's filled strip could not be told from it at 17px.
    expect(dots('rail'), 'the section icons are in the column').toBeGreaterThan(1);
    expect(dots('open')).toBe(0);
    expect(dots('hidden')).toBe(0);
  });

  it('tells a screen reader whether each panel is showing', () => {
    const shown = bar().container;
    const hidden = bar({ leftPanel: 'hidden', rightPanel: false }).container;
    const pressed = (root: HTMLElement, label: string) =>
      root.querySelector(`[aria-label="${label}"]`)?.getAttribute('aria-pressed');

    expect(pressed(shown, 'Left panel')).toBe('true');
    expect(pressed(hidden, 'Left panel')).toBe('false');
    expect(pressed(shown, 'Right panel')).toBe('true');
    expect(pressed(hidden, 'Right panel')).toBe('false');
  });

  it('says what pressing it will do, rather than what the panel is', () => {
    // A button that says "hide" while the thing is already hidden is worse than one that says
    // nothing at all.
    const title = (root: HTMLElement, label: string) =>
      root.querySelector(`[aria-label="${label}"]`)?.getAttribute('title') ?? '';

    // The left panel has three states, so its button is a cycle: open minimises, minimised
    // hides, hidden opens. Three presses come back to where they started.
    expect(title(bar().container, 'Left panel')).toMatch(/^Minimise the left panel/);
    expect(title(bar({ leftPanel: 'rail' }).container, 'Left panel')).toMatch(/^Hide the left panel/);
    expect(title(bar({ leftPanel: 'hidden' }).container, 'Left panel')).toMatch(/^Show the left panel/);
    expect(title(bar().container, 'Right panel')).toMatch(/^Hide the right panel/);
    expect(title(bar().container, 'Left panel')).toContain('(Ctrl \\)');
    expect(title(bar().container, 'Right panel')).toContain('(Ctrl K)');
  });

  it('asks the window to fold a panel away, by the name the keyboard uses', () => {
    const onAction = vi.fn();
    const { container } = bar({ onAction });
    (container.querySelector('[aria-label="Left panel"]') as HTMLButtonElement).click();
    (container.querySelector('[aria-label="Right panel"]') as HTMLButtonElement).click();
    expect(onAction.mock.calls.map((c) => c[0])).toEqual(['panel.left', 'panel.right']);
  });

  it('stops offering the right panel while the conflict tool has the window', () => {
    // It is suppressed for as long as a merge is stopped, so the button would be a control
    // that visibly does nothing.
    const { container } = bar({ rightPanelUsable: false });
    const right = container.querySelector('[aria-label="Right panel"]') as HTMLButtonElement;
    expect(right.disabled).toBe(true);
    expect(right.title).toContain('conflict tool');
  });

  it('leaves the two panel toggles unlit, and lights only the terminal', () => {
    // Three lit buttons in the state the window ships in is a toolbar that shouts.
    const { container } = bar({ terminalOpen: true });
    const lit = [...container.querySelectorAll('button.action.on')]
      .map((b) => b.getAttribute('aria-label'));
    expect(lit).toEqual(['Terminal']);
  });

  it('shows the way out of a submodule on the crumb it belongs to', () => {
    const { container } = bar({ submodule: 'vendor/thing' });
    expect(container.querySelector('.step.sub .value')?.textContent).toBe('thing');
    expect(container.querySelector('.step.sub .leave')).not.toBeNull();
  });
});

describe('where the buttons are', () => {
  /**
   * The actions begin where the breadcrumb ends, so a breadcrumb sized to its text moved every
   * button whenever the branch name did. Checking out `feature/hand-test` after `main` slid
   * Fetch, Pull, Push and the rest more than two button widths to the right, and a hand
   * reaching for one pressed another. happy-dom does not lay out, so this reads the rule.
   */
  it('does not let the branch name move them', () => {
    const { container } = bar();
    const rule = [...document.styleSheets]
      .flatMap((sheet) => [...(sheet.cssRules ?? [])])
      .map((r) => r.cssText)
      .find((text) => /\.where[^{]*\{/u.test(text) && /flex-basis/u.test(text));
    expect(rule, 'the breadcrumb has a width of its own').toBeDefined();
    expect(rule).toMatch(/flex-grow:\s*0/u);
    expect(rule, 'and clips rather than growing').toMatch(/overflow:\s*hidden/u);
    // A basis in pixels, not `auto`: `auto` is the content's width, which is the fault. The
    // pixels come from the element, since a submodule needs a third step's worth of them.
    const where = container.querySelector('.where') as HTMLElement;
    expect(where.style.getPropertyValue('--crumb')).toMatch(/^\d+px$/u);
  });

  /**
   * A submodule adds a third step and a way out of it, and the crumb's fixed width was set for
   * two. All three labels came out cut — "REPOSIT…", "SUBMOD…", "BRAN…" — which reads as
   * broken rather than as an elision.
   *
   * The width still does not follow the branch name; it follows whether a submodule is open,
   * which is a step the reader took deliberately and expects the row to answer.
   */
  it('makes room for the third step, and only for that', () => {
    const width = (over: Record<string, unknown>) => {
      cleanup();
      const el = bar(over).container.querySelector('.where') as HTMLElement;
      return el.style.getPropertyValue('--crumb');
    };
    const plain = width({ submodule: null });
    const inside = width({ submodule: 'books/journal' });
    expect(plain).toMatch(/^\d+px$/u);
    expect(Number.parseInt(inside, 10)).toBeGreaterThan(Number.parseInt(plain, 10));
    expect(width({ submodule: null, branch: 'feature/a-very-long-branch-name' })).toBe(plain);
  });

  it('still names the whole of what it elides', () => {
    // The tooltip is where the full path and the full branch name go.
    const { container } = bar({ path: '/home/dev/a-very-long-directory-name/coral' });
    const steps = [...container.querySelectorAll('.where .step')];
    expect(steps.some((s) => s.getAttribute('title')?.includes('a-very-long-directory-name'))).toBe(
      true,
    );
  });
});

describe('the two buttons that need something to act on', () => {
  /**
   * Pressing Pop with nothing stashed answered "stash@{0} is not a valid reference" — git's
   * own name for a thing the reader never typed, about a stash the panel beside the button
   * already says does not exist.
   */
  function named(container: HTMLElement, label: string): HTMLButtonElement {
    const found = buttons(container).find((b) => b.getAttribute('aria-label') === label);
    if (!found) throw new Error(`no ${label} button`);
    return found;
  }

  it('will not pop when nothing is stashed', () => {
    expect(named(bar({ stashes: 0 }).container, 'Pop').disabled).toBe(true);
    expect(named(bar({ stashes: 2 }).container, 'Pop').disabled).toBe(false);
  });

  it('says so rather than leaving the reader to guess', () => {
    const title = named(bar({ stashes: 0 }).container, 'Pop').title;
    expect(title).toContain('Nothing stashed');
  });

  it('names which stash it would take when there is one', () => {
    expect(named(bar({ stashes: 3 }).container, 'Pop').title).toContain('latest');
  });

  it('will not stash a working copy with nothing in it', () => {
    // git answers "No local changes to save" and does nothing, which is a button that lies.
    expect(named(bar({ dirty: false }).container, 'Stash').disabled).toBe(true);
    expect(named(bar({ dirty: true }).container, 'Stash').disabled).toBe(false);
    expect(named(bar({ dirty: false }).container, 'Stash').title).toContain('Nothing to stash');
  });

  it('refuses both while something else is running, as every other button does', () => {
    const { container } = bar({ busy: true, stashes: 2, dirty: true });
    expect(named(container, 'Pop').disabled).toBe(true);
    expect(named(container, 'Stash').disabled).toBe(true);
  });
});

describe('undo and redo', () => {
  /** The button with that accessible name. */
  function named(container: HTMLElement, label: string): HTMLButtonElement {
    const found = buttons(container).find((b) => b.getAttribute('aria-label') === label);
    if (!found) throw new Error(`no ${label} button`);
    return found;
  }

  it('are off when the journal has nothing at that end, and say so', () => {
    // Offered whatever the journal held, pressing one on a repository nothing had happened in
    // answered "cannot redo: nothing to redo" from a control that had looked available.
    const { container } = bar({ journal: { undo: null, redo: null } });
    expect(named(container, 'Undo').disabled).toBe(true);
    expect(named(container, 'Redo').disabled).toBe(true);
    expect(named(container, 'Undo').title).toContain('Nothing to undo');
    expect(named(container, 'Redo').title).toContain('Nothing to redo');
  });

  it('name what they would act on', () => {
    const { container } = bar({ journal: { undo: 'merge topic', redo: null } });
    expect(named(container, 'Undo').disabled).toBe(false);
    expect(named(container, 'Undo').title).toContain('merge topic');
    expect(named(container, 'Redo').disabled).toBe(true);
  });
});
