import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { fitPanel, fitSubmenu } from '../src/app/placement';

const view = { width: 1440, height: 900, margin: 8 };
const row = (left: number, right: number, top: number) => ({
  left,
  right,
  top,
  bottom: top + 26,
});

describe('fitting a menu into the window', () => {
  it('opens it where it was asked for when there is room', () => {
    expect(fitPanel(300, 200, { width: 240, height: 400 }, view)).toEqual({
      left: 300,
      top: 200,
    });
  });

  it('pulls it back from the right and bottom edges', () => {
    // A menu opened near the bottom would otherwise have its last items — the destructive
    // ones — below the window and unreachable.
    expect(fitPanel(1400, 880, { width: 240, height: 400 }, view)).toEqual({
      left: 1192,
      top: 492,
    });
  });

  it('never pushes it off the top or left to make it fit', () => {
    expect(fitPanel(-50, -50, { width: 240, height: 400 }, view)).toEqual({
      left: 8,
      top: 8,
    });
  });
});

describe('fitting a submenu against its row', () => {
  it('hangs it off the right of the row when there is room', () => {
    expect(fitSubmenu(row(900, 1140, 300), { width: 220, height: 90 }, view)).toEqual({
      left: 'calc(100% - 2px)',
      top: '-5px',
    });
  });

  it('flips it to the left of the row when there is not', () => {
    // The fault this exists for: the commit menu opened against the right edge, its Reset
    // submenu hung off the right of that, and the three choices — which differ only in how
    // much they throw away — were cut off by the window with no way to read them.
    expect(fitSubmenu(row(1180, 1420, 300), { width: 220, height: 90 }, view)).toEqual({
      left: '-218px',
      top: '-5px',
    });
  });

  it('treats a submenu that ends exactly on the margin as fitting', () => {
    expect(
      fitSubmenu(row(1000, 1214, 300), { width: 220, height: 90 }, view).left,
    ).toBe('calc(100% - 2px)');
  });

  it('lifts one that would run off the bottom', () => {
    // A long submenu opened from a row near the foot of the window: shifted up by exactly the
    // amount that was over the edge, so its last item lands on the margin.
    expect(fitSubmenu(row(400, 640, 700), { width: 220, height: 300 }, view).top).toBe('-108px');
  });

  it('leaves one that ends on the margin where it is', () => {
    expect(fitSubmenu(row(400, 640, 600), { width: 220, height: 297 }, view).top).toBe('-5px');
  });
});

describe('what a menu row gives up when it will not fit', () => {
  /**
   * The hint repeats what the row already says and is on the tooltip either way; the label is
   * the row. Shrunk in proportion, as they were, "Fast-forward renamed-branch to main" came
   * out as "Fast-forward renamed-branch to m…" beside a hint that still had room — on the rows
   * that decide how much gets thrown away.
   */
  it('gives the label a floor the hint does not have', () => {
    const css = readFileSync(
      new URL('../src/app/Menu.svelte', import.meta.url),
      'utf8',
    );
    const hint = css.slice(css.indexOf('.hint {'), css.indexOf('.hint {') + 200);
    expect(css, 'a usable row keeps a readable label').toMatch(
      /\.row:not\(:disabled\) \.label \{ min-width: min\(/,
    );
    expect(hint, 'the hint has no floor, so it goes first').toMatch(/min-width:\s*0/);
    expect(hint, 'and it gives way faster than the label').toMatch(/flex:\s*0\s+8\s/);
    expect(css, 'except where the hint is the reason the row is off').toMatch(
      /\.row:disabled \.hint \{ flex-shrink: 0; \}/,
    );
  });
});
