import { invoke } from './invoke';
import type { Dialect, MessageSpelling } from './types';

/** The words of each field the dictionary does not know, in UTF-16 offsets from its start. */
export function checkSpelling(
  summary: string,
  body: string,
  dialect: Dialect,
): Promise<MessageSpelling> {
  return invoke<MessageSpelling>('spelling_check', { summary, body, dialect });
}

/** What a misspelt word might have meant, likeliest first and at most five. */
export function suggestSpelling(word: string, dialect: Dialect): Promise<string[]> {
  return invoke<string[]>('spelling_suggest', { word, dialect });
}

/** Adds a word to the list the window and `coral spell` both accept. */
export function learnWord(word: string): Promise<void> {
  return invoke<void>('spelling_learn', { word });
}
