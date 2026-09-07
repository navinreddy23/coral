import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * The terminal's own sixteen colours.
 *
 * xterm 6 wants the whole set. Given eight of them — the interface's lane colours, lent to the
 * slots whose names happened to match — it did not fall back to its own defaults for the other
 * eight: the palette collapsed and every colour a shell emitted came out as the foreground.
 * A prompt, `ls --color`, a diff, all of it monochrome on a light ground.
 */
const ANSI = [
  'black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white',
  'brightBlack', 'brightRed', 'brightGreen', 'brightYellow',
  'brightBlue', 'brightMagenta', 'brightCyan', 'brightWhite',
] as const;

const component = readFileSync(resolve(process.cwd(), 'src/app/Terminal.svelte'), 'utf8');
const tokens = readFileSync(resolve(process.cwd(), 'src/styles/tokens.css'), 'utf8');

/** The `slot: '--token'` pairs the component hands to xterm. */
function wanted(): Map<string, string> {
  const block = /const wanted: Record<string, string> = \{([\s\S]*?)\};/.exec(component);
  expect(block, 'the palette map is still where this test looks for it').not.toBeNull();
  const pairs = new Map<string, string>();
  for (const line of (block?.[1] ?? '').split('\n')) {
    const m = /^\s*([A-Za-z]+):\s*'(--[a-z0-9-]+)'/.exec(line);
    if (m) pairs.set(m[1], m[2]);
  }
  return pairs;
}

/**
 * Every custom property a theme leaves defined.
 *
 * Light is the bare `:root`. Dark is `:root[data-theme='dark']`, which redefines only what
 * differs, so what it leaves defined is its own block plus everything light set.
 */
function definedIn(theme: 'light' | 'dark'): Set<string> {
  const selector = theme === 'dark' ? /^:root\[data-theme=.dark.\]\s*\{$/m : /^:root\s*\{$/m;
  const at = selector.exec(tokens);
  expect(at, `a ${theme} block`).not.toBeNull();
  const from = (at?.index ?? 0) + (at?.[0].length ?? 0);
  const to = tokens.indexOf('\n}', from);
  const block = tokens.slice(from, to);
  const names = [...block.matchAll(/^\s*(--[a-z0-9-]+):/gm)].map((m) => m[1]);
  return theme === 'dark' ? new Set([...definedIn('light'), ...names]) : new Set(names);
}

describe('the terminal palette', () => {
  it('names all sixteen colours a terminal addresses', () => {
    const slots = wanted();
    for (const name of ANSI) {
      expect(slots.has(name), `${name} is handed to xterm`).toBe(true);
    }
  });

  it('also names the ground it draws on', () => {
    const slots = wanted();
    for (const name of ['background', 'foreground', 'cursor']) {
      expect(slots.has(name), name).toBe(true);
    }
  });

  it('defines every token it asks for, in both themes', () => {
    const asked = [...wanted().values()];
    for (const theme of ['light', 'dark'] as const) {
      const have = definedIn(theme);
      const absent = asked.filter((token) => !have.has(token));
      expect(absent, `undefined in the ${theme} theme`).toEqual([]);
    }
  });

  it('gives the terminal colours of its own rather than the interface lanes', () => {
    // The lane colours are chosen to tell branches apart on a graph, not to be somebody's red.
    const asked = [...wanted().values()];
    expect(asked.filter((t) => t.startsWith('--lane-'))).toEqual([]);
  });
});
