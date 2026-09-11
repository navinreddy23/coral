import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { componentStyles, tokens } from './stylesheet';

/**
 * The rules the whole interface is drawn by, as assertions rather than as good intentions.
 *
 * Three of these are about consistency and one is about rendering. The rendering one is the
 * important one: WebKit antialiases text on a composited layer with subpixel precision only
 * where it knows what is behind it, so a surface carrying text and no background of its own
 * silently drops to grayscale and reads soft. That is invisible in a screenshot taken in a
 * browser and obvious in the real window, which is exactly the kind of fault a test should
 * be holding rather than a person.
 */
const STYLES = componentStyles();

/** Components that float free of the page, and may therefore cast a shadow. */
const OVERLAYS = new Set([
  'Menu.svelte', 'Ask.svelte', 'Palette.svelte', 'Toasts.svelte', 'HostAccount.svelte',
  'RebasePicker.svelte', 'Shortcuts.svelte', 'TabBar.svelte', 'DiffView.svelte',
]);

/** Literal colours that are correct and say so where they are written. */
const ALLOWED_LITERALS = new Set([
  // The letters inside a commit node's disc. White in both themes, which is the whole reason
  // the node palette is dark in both.
  '#ffffff',
]);

describe('the component stylesheets', () => {
  it('clamps a toast to whole lines, so none of them is sliced across the middle', () => {
    // 4.5em against a 1.4 line height is three lines and a fifth of a fourth, which draws the
    // tops of the next line's letters along the bottom edge and looks like a rendering fault.
    // git reports a paragraph on a conflict, so this clamp is reached often.
    const css = STYLES.find((s) => s.name === 'Toasts.svelte')?.css ?? '';
    const rule = /\.detail[^{]*\{([^}]*)\}/u.exec(css);
    expect(rule).not.toBeNull();
    const body = rule?.[1] ?? '';
    expect(body).toMatch(/line-clamp:\s*\d+/u);
    expect(body).not.toMatch(/max-height/u);
  });

  it('write no colour that is not a token', () => {
    const offenders: string[] = [];
    for (const { name, css } of STYLES) {
      for (const found of css.matchAll(/#[0-9a-f]{3,8}\b/gi)) {
        if (!ALLOWED_LITERALS.has(found[0].toLowerCase())) offenders.push(`${name}: ${found[0]}`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('write no type size that is not on the scale', () => {
    // Eighty-three components wrote `12px`, sixty-one wrote `11px`, and five different sizes
    // were used for what was the same heading in five panels.
    const offenders: string[] = [];
    for (const { name, css } of STYLES) {
      for (const found of css.matchAll(/font-size:\s*(\d+)px/g)) {
        offenders.push(`${name}: ${found[1]}px`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('cast a shadow only where something floats above the page', () => {
    // A shadow is a large soft fill. On a row it would be repainted on every scroll frame, and
    // this window scrolls a million rows.
    const offenders: string[] = [];
    for (const { name, css } of STYLES) {
      if (OVERLAYS.has(name)) continue;
      for (const found of css.matchAll(/box-shadow:\s*([^;]+);/g)) {
        const value = found[1] ?? '';
        // An inset shadow is a border drawn on one edge, and a shadow with no blur is a ring
        // round a drop target. Neither is an elevation, and neither costs anything to repaint.
        // `none` takes one away, which is the opposite of what this is looking for.
        if (value.trim() === 'none') continue;
        if (/\binset\b/.test(value) || value.includes('var(--ring)')) continue;
        if (/^0 0 0 /.test(value.trim())) continue;
        offenders.push(`${name}: ${value.trim()}`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('time every transition from the two duration tokens', () => {
    /*
     * So that motion has one switch. `base.css` sets both to nothing under
     * `prefers-reduced-motion`, which turns off every animation in the window at once; a
     * component that writes its own `160ms` opts itself out of that silently.
     *
     * The rendering rule that cannot be checked here is the other half of this file's
     * subject: every surface carrying text has to paint an opaque background of its own or
     * WebKit drops it to grayscale antialiasing. It is not lintable from the CSS text —
     * a row is deliberately transparent so the lane canvas shows through it, and its cells
     * are what paint — so the commit list's version of it is asserted against real rules in
     * `tests/dom/shell.test.ts` instead.
     */
    const offenders: string[] = [];
    for (const { name, css } of STYLES) {
      for (const found of css.matchAll(/transition:[^;]*?(\d+m?s)/g)) {
        offenders.push(`${name}: ${found[1]}`);
      }
    }
    expect(offenders).toEqual([]);
  });
});

describe('a glyph in the middle of a sentence', () => {
  /**
   * `Icon.svelte` renders a block, which is right in every row and button that holds one and
   * wrong inside a paragraph: the line breaks at the glyph and again after it, and the mark is
   * left stranded on a line of its own with the rest of the sentence below it.
   */
  it('is put back in the run of words by whatever wraps it', () => {
    const offenders: string[] = [];
    for (const { name, css, source } of STYLES) {
      // A wrapper inside a paragraph, which is the only place this goes wrong.
      if (!/<p[^>]*>[\s\S]*?<span class="glyph">[\s\S]*?<\/p>/u.test(source)) continue;
      const rule = /\.glyph\s*\{([^}]*)\}/u.exec(css)?.[1] ?? '';
      if (!/display:\s*inline/u.test(rule)) offenders.push(name);
    }
    expect(offenders).toEqual([]);
  });
});

describe('scrollbars while an overlay has the window', () => {
  /**
   * WebKitGTK draws a scroller's bar above page content, so the commit list's thumb ran down
   * the middle of every open context menu and of the command palette — a three-pixel rule
   * across the words. No `z-index` on the overlay reaches it, because the bar is not in the
   * page's stacking order at all, and Chrome orders it correctly, so nothing but the real
   * window ever showed it.
   */
  const BASE = readFileSync(resolve(import.meta.dirname, '../src/styles/base.css'), 'utf8');

  it('are taken away for as long as a scrim is up', () => {
    expect(BASE).toMatch(/body:has\(\.scrim\)[^{]*\{[^}]*scrollbar-width:\s*none/u);
    expect(BASE).toMatch(/body:has\(\.scrim\)[^{]*::-webkit-scrollbar[^{]*\{[^}]*display:\s*none/u);
  });

  it('are keyed on the scrim every overlay already draws', () => {
    // So an overlay added later is covered by being built the way the others are.
    const wearing = componentStyles().filter((c) => /\.scrim\s*[,{]/u.test(c.css));
    expect(wearing.length, 'the overlays that draw one').toBeGreaterThan(4);
  });
});

describe('the token file', () => {
  it('offers a size for lettering inside a shape, outside the reading scale', () => {
    expect(tokens('light').has('text-mark')).toBe(true);
  });
});
