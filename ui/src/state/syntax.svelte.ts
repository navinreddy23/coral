import { highlight } from '../syntax/highlighter';
import type { Grammar } from '../syntax/grammar';
import type { Token } from '../syntax/token';
import type { ViewsState } from './views.svelte';

/**
 * Past these a file is drawn plain.
 *
 * Not because the worker cannot colour it but because nobody reads a file that size for its
 * colours, and holding its tokens costs more than the text: the kernel's MAINTAINERS is a
 * megabyte of plain text with thirty-one thousand lines, which is the size this stops at.
 */
const MOST_BYTES = 1_000_000;
const MOST_LINES = 20_000;

/** Files whose tokens are kept: both sides of a diff, a blame, a file view, and a few back. */
const KEPT = 12;

/**
 * Code colours for whole files, asked of the worker and kept for the files on screen.
 *
 * Keyed by the text itself rather than by where it came from: the working copy's file changes
 * under the same name, and the same text read for a diff, a blame and the file view is coloured
 * once.
 */
export class SyntaxState {
  /**
   * Tokens per file's text, or null while the worker has it or after it declined.
   *
   * The text alone is the key, not joined to the grammar: a joined key copies a megabyte for
   * every look-up, where a string already used as a key has its hash kept.
   */
  #kept = new Map<string, { grammar: Grammar; lines: Token[][] | null }>();
  /** Bumped as answers land, so what reads the tokens draws again. */
  #landed = $state(0);
  #views: ViewsState;

  constructor(views: ViewsState) {
    this.#views = views;
  }

  /** Whether code is coloured at all, which Appearance turns off. */
  get on(): boolean {
    return this.#views.current.syntax;
  }

  /** A file's tokens, one list per line, once they are back; null to draw it plain. */
  tokens(grammar: Grammar | null, text: string | null): Token[][] | null {
    void this.#landed;
    if (!this.on || grammar === null || text === null) return null;
    const held = this.#kept.get(text);
    return held?.grammar === grammar ? held.lines : null;
  }

  /** Asks for a file's colours, once, unless it is too large to be worth colouring. */
  want(grammar: Grammar | null, text: string | null): void {
    if (!this.on || grammar === null || text === null || !worthColouring(text)) return;
    const held = this.#kept.get(text);
    if (held?.grammar === grammar) {
      // Seen again: moved to the back, so the files on screen are the last to be let go.
      this.#kept.delete(text);
      this.#kept.set(text, held);
      return;
    }
    const entry: { grammar: Grammar; lines: Token[][] | null } = { grammar, lines: null };
    this.#kept.set(text, entry);
    while (this.#kept.size > KEPT) {
      const oldest = this.#kept.keys().next().value;
      if (oldest === undefined) break;
      this.#kept.delete(oldest);
    }
    void highlight(grammar, text).then((lines) => {
      if (this.#kept.get(text) !== entry) return;
      entry.lines = lines;
      this.#landed += 1;
    });
  }
}

function worthColouring(text: string): boolean {
  if (text.length > MOST_BYTES) return false;
  let lines = 0;
  for (let at = text.indexOf('\n'); at !== -1; at = text.indexOf('\n', at + 1)) {
    if (++lines > MOST_LINES) return false;
  }
  return true;
}
