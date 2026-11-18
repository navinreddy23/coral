import type { Misspelling } from '../ipc/types';

/** A run of a field's text, and whether it is a misspelt word. */
export interface Run {
  text: string;
  /** Where it starts in the text, in UTF-16 units; a marked run is found again by this. */
  start: number;
  marked: boolean;
}

/** The text cut into plain and marked runs, in order, with nothing left out. */
export function runs(text: string, marks: Misspelling[]): Run[] {
  const out: Run[] = [];
  let at = 0;
  for (const mark of [...marks].sort((a, b) => a.start - b.start)) {
    if (mark.start < at || mark.end > text.length) continue;
    if (mark.start > at) out.push({ text: text.slice(at, mark.start), start: at, marked: false });
    out.push({ text: text.slice(mark.start, mark.end), start: mark.start, marked: true });
    at = mark.end;
  }
  if (at < text.length) out.push({ text: text.slice(at), start: at, marked: false });
  return out;
}

/**
 * Everything about a field that decides where its text falls, copied to the layer drawn over
 * it so each word lands on the word beneath. The list follows component/textarea-caret-position,
 * which measures a caret the same way.
 */
export const COPIED = [
  'direction',
  'box-sizing',
  'border-top-width',
  'border-right-width',
  'border-bottom-width',
  'border-left-width',
  'font-style',
  'font-variant',
  'font-weight',
  'font-stretch',
  'font-size',
  'font-size-adjust',
  'line-height',
  'font-family',
  'font-feature-settings',
  'font-variation-settings',
  'font-kerning',
  'text-align',
  'text-transform',
  'text-indent',
  'text-rendering',
  'letter-spacing',
  'word-spacing',
  'tab-size',
] as const;
