import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

import { blocks, declarations, token } from './stylesheet';

/**
 * The copies of the palette that live outside the token file, and cannot be helped.
 *
 * Two things paint before anything that could read a custom property exists. The native
 * window paints `backgroundColor` from `tauri.conf.json` before the webview has started, and
 * the inline block in `index.html` paints the boot screen before the stylesheet has arrived
 * — the content policy forbids the inline script that would otherwise read the tokens.
 *
 * So the values are copied by hand, and this is what keeps the copies honest. Without it the
 * window opened as a white rectangle on a dark desktop for as long as the bundle took.
 */
const HTML = readFileSync(resolve(import.meta.dirname, '../index.html'), 'utf8');
const CONFIG = readFileSync(
  resolve(import.meta.dirname, '../../crates/coral-app/tauri.conf.json'),
  'utf8',
);

/** The inline stylesheet in the head, which is the only CSS the boot screen has. */
const INLINE = /<style>([\s\S]*?)<\/style>/.exec(HTML)?.[1] ?? '';

/** Every colour written into a block whose selectors match, in source order. */
function coloursIn(within: (selector: string) => boolean): string[] {
  const out: string[] = [];
  for (const [selector, body] of blocks(INLINE)) {
    if (!within(selector)) continue;
    for (const found of body.matchAll(/#[0-9a-f]{3,8}\b/gi)) out.push(found[0].toLowerCase());
  }
  return out;
}

/** The dark half of the inline sheet, which is everything inside its media query. */
const darkStart = INLINE.indexOf('@media (prefers-color-scheme: dark)');
const light = INLINE.slice(0, darkStart);
const dark = INLINE.slice(darkStart);

function coloursOf(half: string): Set<string> {
  const out = new Set<string>();
  for (const found of half.matchAll(/#[0-9a-f]{3,8}\b/gi)) out.add(found[0].toLowerCase());
  return out;
}

describe('the boot screen', () => {
  it('paints the page in the light theme panel colour', () => {
    expect(coloursOf(light)).toContain(token('light', 'bg-1'));
  });

  it('paints the page in the dark theme page colour when the desktop is dark', () => {
    // The dark half names the page rather than the panel: a dark window that opens on the
    // panel colour and settles onto the page colour reads as a flicker, and the step between
    // the two is wider in dark than in light.
    expect(coloursOf(dark)).toContain(token('dark', 'bg-0'));
  });

  it('writes the word in the dim foreground of whichever theme is showing', () => {
    expect(coloursOf(light)).toContain(token('light', 'fg-2'));
    expect(coloursOf(dark)).toContain(token('dark', 'fg-2'));
  });

  it('draws the mark in the brand colour, which is what a mark is for', () => {
    expect(coloursOf(light)).toContain(token('light', 'brand'));
    expect(coloursOf(dark)).toContain(token('dark', 'brand'));
  });

  it('uses no colour that is not a token', () => {
    // The point of the test: a hand-written value here drifts silently, because nothing
    // renders both files together until the application is launched.
    const known = new Set(
      (['light', 'dark'] as const).flatMap((theme) =>
        ['bg-0', 'bg-1', 'fg-2', 'brand', 'accent'].map((name) => token(theme, name)),
      ),
    );
    for (const colour of coloursIn(() => true)) {
      expect(known, `${colour} in index.html is not a token value`).toContain(colour);
    }
  });

  it('asks for the bundled interface face before the system one', () => {
    // Once the fonts are cached the boot word is set in the same face as the window it opens
    // into, rather than changing shape the moment the bundle lands.
    expect(INLINE).toContain("'Inter Variable'");
  });
});

describe('the native window', () => {
  it('opens on the same colour the page paints', () => {
    // Tauri paints this before the webview exists. It cannot follow the desktop's theme from
    // a static config, so it holds the light value; `index.html` takes over within a frame.
    const found = /"backgroundColor"\s*:\s*"(#[0-9a-f]{3,8})"/i.exec(CONFIG)?.[1]?.toLowerCase();
    expect(found).toBe(token('light', 'bg-1'));
  });
});

describe('the inline sheet', () => {
  it('declares no custom property, since nothing has loaded that could define one', () => {
    expect(declarations(INLINE)).toHaveLength(0);
  });
});
