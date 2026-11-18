import type { Span } from '../diff/word';
import type { GrepPattern } from '../ipc/types';

/**
 * Where a search's pattern is on a line it found, as stretches to mark.
 *
 * Found again here rather than taken from git, which says only where the first match starts,
 * and in bytes. Text is matched as text, through an expression with everything escaped, so a
 * case-insensitive match never counts in a lower-cased copy whose length can differ. An
 * expression JavaScript cannot read — git's dialect is not quite its own — leaves the line
 * unmarked rather than marked wrong.
 */
export function hitSpans(
  text: string,
  pattern: string,
  how: GrepPattern,
  ignoreCase: boolean,
): Span[] | null {
  if (pattern === '') return null;
  const source = how === 'fixed' ? pattern.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&') : pattern;
  let expression: RegExp;
  try {
    expression = new RegExp(source, ignoreCase ? 'giu' : 'gu');
  } catch {
    return null;
  }
  const spans: Span[] = [];
  let at = 0;
  for (const match of text.matchAll(expression)) {
    // An empty match marks nothing and would only cut the line up.
    if (match[0] === '') continue;
    if (match.index > at) spans.push({ text: text.slice(at, match.index), marked: false });
    spans.push({ text: match[0], marked: true });
    at = match.index + match[0].length;
  }
  if (spans.length === 0) return null;
  if (at < text.length) spans.push({ text: text.slice(at), marked: false });
  return spans;
}
