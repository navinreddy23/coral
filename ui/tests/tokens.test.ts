import { describe, expect, it } from 'vitest';

import { TOKENS_CSS, blocks, declarations, tokens } from './stylesheet';

/**
 * The shape of the token file, as opposed to the colours in it.
 *
 * Three of these are contracts with code that lives elsewhere and would fail silently at
 * runtime rather than loudly here: the lane names are persisted to disk by
 * `crates/coral-app/src/session.rs`, the same names are turned back into `var()` strings by
 * `ui/src/app/lane.ts`, and the terminal names are read out of the computed style by
 * `ui/src/app/Terminal.svelte`.
 */

/** A value that names a colour, as opposed to a length, a family or a duration. */
const isColour = (value: string) => /^(#[0-9a-f]{3,8}|rgb\(|color-mix\()/i.test(value);

/**
 * Colour tokens that are deliberately the same in both themes, and why.
 *
 * Anything not on this list must be redefined for dark. A colour that is only declared once
 * is the classic unreadable-theme bug: it was chosen against one ground and is then painted
 * on the other.
 */
const SHARED = new Set([
  // The letters inside a commit node are white in both themes, so the fill under them has to
  // be dark in both. The dark lane palette is light, and white on it is 1.5:1.
  'node-1', 'node-2', 'node-3', 'node-4', 'node-5', 'node-6', 'node-7', 'node-8',
]);

describe('the token file', () => {
  it('defines the eight lane colours and their tints, which are stored on disk by name', () => {
    for (const theme of ['light', 'dark'] as const) {
      const has = tokens(theme);
      for (let lane = 1; lane <= 8; lane++) {
        expect(has.has(`lane-${lane}`), `--lane-${lane} in ${theme}`).toBe(true);
        expect(has.has(`lane-${lane}-soft`), `--lane-${lane}-soft in ${theme}`).toBe(true);
      }
    }
  });

  it('defines the eight node fills, which the canvas reads by name', () => {
    for (let node = 1; node <= 8; node++) {
      expect(tokens('light').has(`node-${node}`)).toBe(true);
    }
  });

  it('defines the sixteen colours a terminal addresses', () => {
    const hues = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];
    for (const theme of ['light', 'dark'] as const) {
      for (const hue of hues) {
        expect(tokens(theme).has(`term-${hue}`), `--term-${hue} in ${theme}`).toBe(true);
        expect(tokens(theme).has(`term-bright-${hue}`), `--term-bright-${hue}`).toBe(true);
      }
    }
  });

  it('redefines every colour for the dark theme', () => {
    const dark = new Set(
      blocks(TOKENS_CSS)
        .filter(([selector]) => selector.includes("data-theme='dark'"))
        .flatMap(([, body]) => declarations(body).map(([name]) => name)),
    );
    const missing: string[] = [];
    for (const [name, value] of tokens('light')) {
      if (!isColour(value) || SHARED.has(name) || dark.has(name)) continue;
      missing.push(name);
    }
    expect(missing, 'colours declared for light only').toEqual([]);
  });

  it('defines nothing for dark that light has never heard of', () => {
    // A token that exists in one theme only renders as an empty string in the other, which
    // CSS treats as an invalid declaration and simply drops.
    const light = tokens('light');
    for (const [selector, body] of blocks(TOKENS_CSS)) {
      if (!selector.includes("data-theme='dark'")) continue;
      for (const [name] of declarations(body)) {
        if (name === 'color-scheme') continue;
        expect(light.has(name), `--${name} is dark-only`).toBe(true);
      }
    }
  });

  it('declares each token once per block', () => {
    for (const [selector, body] of blocks(TOKENS_CSS)) {
      const seen = new Set<string>();
      for (const [name] of declarations(body)) {
        expect(seen.has(name), `--${name} is declared twice in ${selector}`).toBe(false);
        seen.add(name);
      }
    }
  });

  it('names the bundled faces ahead of the system stack', () => {
    // Named ahead of it rather than instead of it, so a build that somehow ships without the
    // woff2 files still renders something.
    expect(tokens('light').get('font-ui')).toMatch(/^'Inter Variable', Inter, system-ui/);
    expect(tokens('light').get('font-mono')).toMatch(/^'JetBrains Mono', ui-monospace/);
  });

  it('gives every density a row height, and the default one the middle of them', () => {
    const heights = new Map<string, string>();
    for (const [selector, body] of blocks(TOKENS_CSS)) {
      const density = /data-density='([a-z]+)'/.exec(selector)?.[1];
      const found = declarations(body).find(([name]) => name === 'row-h');
      if (found) heights.set(density ?? 'default', found[1]);
    }
    expect([...heights.keys()].sort()).toEqual(['comfortable', 'compact', 'default']);
    const px = (name: string) => Number.parseInt(heights.get(name) ?? '', 10);
    expect(px('compact')).toBeLessThan(px('default'));
    expect(px('default')).toBeLessThan(px('comfortable'));
  });

  it('sets a type scale in order, with no size defined twice', () => {
    const sizes = ['text-xs', 'text-sm', 'text-base', 'text-md', 'text-lg', 'text-xl'].map(
      (name) => Number.parseInt(tokens('light').get(name) ?? '', 10),
    );
    expect(sizes.some(Number.isNaN)).toBe(false);
    expect([...sizes].sort((a, b) => a - b)).toEqual(sizes);
    expect(new Set(sizes).size).toBe(sizes.length);
  });

  it('offers one shadow for what rests on the page and one for what floats above it', () => {
    for (const theme of ['light', 'dark'] as const) {
      expect(tokens(theme).has('elevate-1'), theme).toBe(true);
      expect(tokens(theme).has('elevate-2'), theme).toBe(true);
      expect(tokens(theme).has('scrim'), theme).toBe(true);
    }
  });

  it('keeps type, spacing and motion out of the theme blocks', () => {
    // A theme changes what something is coloured, never how large it is or how fast it
    // moves. Putting a size in a theme block is how two themes stop being the same window.
    const shapes = /^(text|leading|space|radius|font|row-h|fast|slow|ease)/;
    for (const [selector, body] of blocks(TOKENS_CSS)) {
      if (!selector.includes('data-theme')) continue;
      for (const [name] of declarations(body)) {
        expect(shapes.test(name), `--${name} is themed but is not a colour`).toBe(false);
      }
    }
  });
});
