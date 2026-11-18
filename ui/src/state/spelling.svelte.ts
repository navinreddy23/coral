import { checkSpelling, learnWord, suggestSpelling } from '../ipc/spelling';
import { messageOf } from '../ipc/error';
import type { Dialect, Misspelling } from '../ipc/types';
import type { ViewsState } from './views.svelte';

/** The two fields of the commit box. */
export type Field = 'summary' | 'body';

/**
 * How long typing has to pause before the message is checked. Long enough that a word is not
 * checked once per letter, short enough that the underline arrives as the eye gets there.
 */
export const PAUSE_MS = 250;

/** A character that makes the letters either side of it one word. */
const JOINS = /[\p{L}\p{N}_'’]/u;

/**
 * The marks of `before`, moved to where they are in `after`.
 *
 * The underline has to follow the text between one check and the next, or it lags a word
 * behind the typing. A mark is kept only if the same word still stands where it moved to,
 * whole: typing on from the end of a word, or deleting the space after it, makes it a
 * different word, which the next check will judge.
 */
export function carry(marks: Misspelling[], before: string, after: string): Misspelling[] {
  if (before === after) return marks;
  const shortest = Math.min(before.length, after.length);
  let prefix = 0;
  while (prefix < shortest && before[prefix] === after[prefix]) prefix += 1;
  let suffix = 0;
  while (
    suffix < shortest - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  ) {
    suffix += 1;
  }
  const editEnd = before.length - suffix;
  const delta = after.length - before.length;
  return marks.flatMap((mark) => {
    if (mark.start < editEnd && mark.end > prefix) return [];
    const shift = mark.start >= editEnd ? delta : 0;
    const moved = { ...mark, start: mark.start + shift, end: mark.end + shift };
    return standsWhole(moved, after) ? [moved] : [];
  });
}

function standsWhole(mark: Misspelling, text: string): boolean {
  const before = text[mark.start - 1] ?? ' ';
  const after = text[mark.end] ?? ' ';
  return text.slice(mark.start, mark.end) === mark.word && !JOINS.test(before) && !JOINS.test(after);
}

/**
 * The commit box's spelling: which words of each field to underline, kept current as the
 * message changes and checked again whenever typing pauses.
 *
 * Held by the window rather than the panel, like the message itself, so a word ignored stays
 * ignored when the panel is put away and brought back.
 */
export class SpellingState {
  #marks = $state<Record<Field, Misspelling[]>>({ summary: [], body: [] });
  /** Words ignored for the rest of this session, in lowercase. */
  #ignored = $state<string[]>([]);
  /** Where the last keystroke left the caret, while the word there is still being typed. */
  #typing = $state<{ field: Field; at: number } | null>(null);
  #text: Record<Field, string> = { summary: '', body: '' };
  #checkedIn: Dialect | null = null;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #composing = false;
  #sequence = 0;
  #views: ViewsState;
  #onError: (message: string) => void;

  constructor(views: ViewsState, onError: (message: string) => void = () => undefined) {
    this.#views = views;
    this.#onError = onError;
  }

  /** The English the message is checked in, or null when checking is off. */
  get dialect(): Dialect | null {
    const chosen = this.#views.current.spelling;
    return chosen === 'off' ? null : chosen;
  }

  /** The message as it now stands, from typing or from anything that filled it in. */
  update(summary: string, body: string): void {
    const dialect = this.dialect;
    const same = summary === this.#text.summary && body === this.#text.body;
    if (same && dialect === this.#checkedIn) return;
    this.#marks =
      dialect === this.#checkedIn
        ? {
            summary: carry(this.#marks.summary, this.#text.summary, summary),
            body: carry(this.#marks.body, this.#text.body, body),
          }
        : { summary: [], body: [] };
    this.#text = { summary, body };
    this.#checkedIn = dialect;
    this.#schedule();
  }

  /** The words to underline in one field. */
  shown(field: Field): Misspelling[] {
    const typing = this.#typing;
    return this.#marks[field].filter(
      (mark) =>
        !this.#ignored.includes(mark.word.toLowerCase()) &&
        !(typing?.field === field && typing.at === mark.end),
    );
  }

  /** A keystroke in `field` left the caret at `caret`. */
  typed(field: Field, caret: number): void {
    this.#typing = { field, at: caret };
  }

  /** The caret moved without typing: the word it left is finished with. */
  moved(field: Field, caret: number): void {
    const typing = this.#typing;
    if (typing && (typing.field !== field || typing.at !== caret)) this.#typing = null;
  }

  left(): void {
    this.#typing = null;
  }

  /** An input method is building a character; checking half of one is checking nothing. */
  composing(on: boolean): void {
    this.#composing = on;
    if (!on) this.#schedule();
  }

  ignore(word: string): void {
    const lower = word.toLowerCase();
    if (!this.#ignored.includes(lower)) this.#ignored = [...this.#ignored, lower];
  }

  async suggest(word: string): Promise<string[]> {
    const dialect = this.dialect;
    if (dialect === null) return [];
    try {
      return await suggestSpelling(word, dialect);
    } catch {
      return [];
    }
  }

  /** Adds the word to the list `coral spell` shares, and checks again with it there. */
  async learn(word: string): Promise<void> {
    try {
      await learnWord(word);
    } catch (e) {
      this.#onError(messageOf(e));
      return;
    }
    await this.#check();
  }

  #schedule(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    if (this.#checkedIn === null) {
      this.#marks = { summary: [], body: [] };
      return;
    }
    if (this.#composing) return;
    this.#timer = setTimeout(() => void this.#check(), PAUSE_MS);
  }

  async #check(): Promise<void> {
    this.#timer = null;
    const dialect = this.#checkedIn;
    if (dialect === null) return;
    this.#sequence += 1;
    const sequence = this.#sequence;
    const { summary, body } = this.#text;
    let found;
    try {
      found = await checkSpelling(summary, body, dialect);
    } catch {
      // Nothing to underline is the honest answer when the check itself failed; saying so on
      // every pause in typing would be worse than the missing underline.
      return;
    }
    if (sequence !== this.#sequence || dialect !== this.#checkedIn) return;
    this.#marks = {
      summary: carry(found.summary, summary, this.#text.summary),
      body: carry(found.body, body, this.#text.body),
    };
  }
}
