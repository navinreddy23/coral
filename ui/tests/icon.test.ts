import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

import { ICONS, strokeFor, type IconName } from '../src/app/icon';

/**
 * The icon set, held to the conventions it is drawn on.
 *
 * A glyph that strays off the grid or out of the safe area does not fail anything at
 * runtime — it just sits half a pixel low beside every other one, and nobody can say why the
 * row looks crooked.
 */
const NAMES = Object.keys(ICONS) as IconName[];

/** Every number in a path's `d` attribute. */
function coordinates(d: string): number[] {
  return [...d.matchAll(/-?(?:\d+\.?\d*|\.\d+)/g)].map(([n]) => Number(n));
}

describe('the icon set', () => {
  it('draws something for every name it offers', () => {
    for (const name of NAMES) {
      const glyph = ICONS[name];
      const drawn = (glyph.paths?.length ?? 0) + (glyph.solid?.length ?? 0);
      expect(drawn, `${name} draws nothing`).toBeGreaterThan(0);
    }
  });

  it('stays inside the 24-unit square', () => {
    // Arc flags and a few sweep parameters are bare 0s and 1s, so this catches a coordinate
    // that has run off the grid rather than proving every number is a position.
    for (const name of NAMES) {
      const glyph = ICONS[name];
      for (const d of [...(glyph.paths ?? []), ...(glyph.solid ?? [])]) {
        for (const value of coordinates(d)) {
          expect(value, `${name} has ${value} in "${d.slice(0, 30)}…"`).toBeGreaterThanOrEqual(-24);
          expect(value, `${name} has ${value}`).toBeLessThanOrEqual(24);
        }
      }
    }
  });

  it('names each glyph once, in lower camel case', () => {
    for (const name of NAMES) expect(name).toMatch(/^[a-z][A-Za-z]*$/);
    expect(new Set(NAMES).size).toBe(NAMES.length);
  });
});

describe('the stroke weight', () => {
  it('grows as a glyph shrinks, so the whole set reads at one optical weight', () => {
    // A fixed width shrinks with the icon: two units is 1.3px at sixteen and 0.9px at eleven,
    // and WebKit renders the second as a smudge.
    expect(strokeFor(11)).toBeGreaterThan(strokeFor(16));
    expect(strokeFor(16)).toBeGreaterThan(strokeFor(24));
  });

  it('lands near a pixel and a third on screen at every ordinary size', () => {
    for (const size of [12, 14, 16, 18, 20]) {
      const onScreen = (strokeFor(size) * size) / 24;
      expect(onScreen, `at ${size}px`).toBeGreaterThan(1.2);
      expect(onScreen, `at ${size}px`).toBeLessThan(1.5);
    }
  });

  it('stops short at each end, so a tiny glyph does not close into a blob', () => {
    expect(strokeFor(4)).toBeLessThanOrEqual(2.8);
    expect(strokeFor(64)).toBeGreaterThanOrEqual(1.6);
  });
});

describe('the components that draw them', () => {
  const sources = readdirSync(resolve(import.meta.dirname, '../src/app'))
    .filter((file) => file.endsWith('.svelte'))
    .map((file) => ({
      file,
      text: readFileSync(resolve(import.meta.dirname, '../src/app', file), 'utf8'),
    }));

  it('only ask for glyphs that exist', () => {
    // A misspelt name is `undefined` at runtime and renders an empty square, silently.
    const known = new Set<string>(NAMES);
    for (const { file, text } of sources) {
      for (const [, name] of text.matchAll(/<Icon\s[^>]*name="([a-zA-Z]+)"/g)) {
        expect(known, `${file} asks for "${name}"`).toContain(name);
      }
    }
  });

  it('leave no typeface glyph standing where a drawn one belongs', () => {
    // These were the controls: `↶` for undo, `⤓` for stash, `▾` for a caret, `✕` for close.
    // Font arrows have no bold cut, which is why the toolbar used to fake one with
    // `-webkit-text-stroke`, and they differ on every machine.
    const banned = /[↶↷⟳↻⤓⤒⑂▾▴▸✕✎⋮☰¶⌄]/;
    for (const { file, text } of sources) {
      const markup = text.slice(text.indexOf('</script>'));
      const found = banned.exec(markup);
      expect(found?.[0], `${file} still draws with the glyph ${found?.[0]}`).toBeUndefined();
    }
  });
});
