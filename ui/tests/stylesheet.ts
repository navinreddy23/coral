/**
 * Reading the stylesheets back, so a test can assert what is in them.
 *
 * The token file is the one place colour lives, which only means anything if something
 * checks. These helpers parse it rather than mounting a component, because a token nobody
 * uses yet still has to be defined for both themes.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');

export const TOKENS_CSS = readFileSync(resolve(ROOT, 'src/styles/tokens.css'), 'utf8');

/** Which theme a token block belongs to. */
export type Theme = 'light' | 'dark';

/**
 * Every custom property declared for a theme, by name without the leading dashes.
 *
 * Light is everything on a bare `:root`, which includes the shared type and spacing blocks.
 * Dark is light with the `[data-theme='dark']` block laid over it, which is exactly how the
 * cascade resolves it in the window.
 */
export function tokens(theme: Theme): Map<string, string> {
  const light = new Map<string, string>();
  const dark = new Map<string, string>();
  for (const [selector, body] of blocks(TOKENS_CSS)) {
    const into = selector.includes("data-theme='dark'") ? dark : light;
    for (const [name, value] of declarations(body)) into.set(name, value);
  }
  if (theme === 'light') return light;
  return new Map([...light, ...dark]);
}

/** A token's value, or a failure naming it — an undefined token reads as an empty string. */
export function token(theme: Theme, name: string): string {
  const found = tokens(theme).get(name);
  if (found === undefined) throw new Error(`--${name} is not defined for the ${theme} theme`);
  return found;
}

/** Every `--name: value` pair in a block of CSS. */
export function declarations(css: string): [string, string][] {
  return [...css.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/g)].map(
    ([, name, value]) => [name ?? '', (value ?? '').trim()],
  );
}

/**
 * Every `selector { body }` pair.
 *
 * Comments are stripped first, because a comment may contain a brace and would otherwise cut
 * a block in half. Nesting is not handled and does not need to be: an at-rule yields the
 * blocks inside it, which is what a scan for declarations wants anyway.
 */
export function blocks(css: string): [string, string][] {
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, '');
  return [...bare.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(
    (match) => [(match[1] ?? '').trim(), match[2] ?? ''],
  );
}

/** Every component that has a `<style>` block, as a name and its CSS. */
export function componentStyles(): { name: string; css: string }[] {
  const out: { name: string; css: string }[] = [];
  for (const dir of ['src/app', 'src/graph']) {
    for (const file of readdirSync(resolve(ROOT, dir))) {
      if (!file.endsWith('.svelte')) continue;
      const source = readFileSync(resolve(ROOT, dir, file), 'utf8');
      const style = /<style>([\s\S]*?)<\/style>/.exec(source);
      if (style?.[1]) out.push({ name: file, css: style[1] });
    }
  }
  return out;
}

/**
 * The WCAG contrast ratio between two `#rrggbb` colours.
 *
 * Written here rather than taken from a package: it is nine lines, and the ratios it
 * produces are the argument for half the values in the token file.
 */
export function contrast(a: string, b: string): number {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (high + 0.05) / (low + 0.05);
}

function luminance(hex: string): number {
  const digits = hex.replace('#', '');
  const full = digits.length === 3 ? [...digits].map((c) => c + c).join('') : digits;
  if (!/^[0-9a-f]{6}$/i.test(full)) throw new Error(`${hex} is not a colour`);
  const [r, g, b] = [0, 2, 4].map((at) => channel(parseInt(full.slice(at, at + 2), 16)));
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}

function channel(byte: number): number {
  const c = byte / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/**
 * How far apart two colours look, in OKLab.
 *
 * A contrast ratio cannot answer this. Two colours of the same lightness measure 1.0:1
 * against each other whether they are both salmon or one is salmon and one is teal, so
 * asking whether the brand can be told apart from the danger colour needs a perceptual
 * distance rather than a luminance one. The conversion is Björn Ottosson's, from
 * https://bottosson.github.io/posts/oklab/, which is public domain.
 */
export function apart(a: string, b: string): number {
  const [l1, a1, b1] = oklab(a);
  const [l2, a2, b2] = oklab(b);
  return Math.hypot(l1 - l2, a1 - a2, b1 - b2);
}

function oklab(hex: string): [number, number, number] {
  const digits = hex.replace('#', '');
  const full = digits.length === 3 ? [...digits].map((c) => c + c).join('') : digits;
  const [r, g, b] = [0, 2, 4].map((at) => channel(parseInt(full.slice(at, at + 2), 16))) as
    [number, number, number];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}
