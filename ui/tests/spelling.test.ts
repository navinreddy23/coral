// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { PAUSE_MS, SpellingState, carry } from '../src/state/spelling.svelte';
import { ViewsState } from '../src/state/views.svelte';
import type { Misspelling } from '../src/ipc/types';

function mark(text: string, word: string, from = 0): Misspelling {
  const start = text.indexOf(word, from);
  return { word, start, end: start + word.length };
}

/** Answers checks the way the engine would, for a fixed set of typos. */
function engine(typos: string[] = ['teh', 'recieve', 'colour']) {
  const found = (text: string) =>
    [...text.matchAll(/\p{L}+/gu)]
      .filter((m) => typos.includes(m[0].toLowerCase()))
      .map((m) => ({ word: m[0], start: m.index, end: m.index + m[0].length }));
  invoke.mockImplementation(async (command: string, args: Record<string, string>) => {
    if (command === 'spelling_check') {
      return { summary: found(args['summary'] ?? ''), body: found(args['body'] ?? '') };
    }
    if (command === 'spelling_suggest') return ['the', 'eh'];
    if (command === 'spelling_learn') {
      typos.splice(typos.indexOf(args['word'] ?? ''), 1);
      return null;
    }
    throw new Error(`unexpected ${command}`);
  });
}

function checks(): number {
  return invoke.mock.calls.filter((c) => c[0] === 'spelling_check').length;
}

/** Lets the pause run out and the check's answer arrive. */
async function pause(): Promise<void> {
  await vi.advanceTimersByTimeAsync(PAUSE_MS);
}

beforeEach(() => {
  localStorage.clear();
  invoke.mockReset();
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('carrying marks through an edit', () => {
  const text = 'Fix teh watcher and recieve it';
  const marks = [mark(text, 'teh'), mark(text, 'recieve')];

  const cases: { name: string; after: string; kept: string[] }[] = [
    { name: 'an edit before both moves both', after: 'Now fix teh watcher and recieve it', kept: ['teh', 'recieve'] },
    { name: 'an edit after both moves neither', after: 'Fix teh watcher and recieve it all', kept: ['teh', 'recieve'] },
    { name: 'an edit between them moves only the second', after: 'Fix teh old watcher and recieve it', kept: ['teh', 'recieve'] },
    { name: 'an edit inside a word drops it', after: 'Fix the watcher and recieve it', kept: ['recieve'] },
    { name: 'typing on from the end of a word drops it', after: 'Fix tehx watcher and recieve it', kept: ['recieve'] },
    { name: 'typing onto the front of a word drops it', after: 'Fix xteh watcher and recieve it', kept: ['recieve'] },
    { name: 'deleting the space after a word drops it', after: 'Fix tehwatcher and recieve it', kept: ['recieve'] },
    { name: 'punctuation after a word keeps it', after: 'Fix teh, watcher and recieve it', kept: ['teh', 'recieve'] },
    { name: 'replacing everything drops everything', after: 'Something else', kept: [] },
  ];
  for (const { name, after, kept } of cases) {
    it(name, () => {
      const moved = carry(marks, text, after);
      expect(moved.map((m) => m.word)).toEqual(kept);
      for (const m of moved) expect(after.slice(m.start, m.end)).toBe(m.word);
    });
  }

  it('keeps the underline when a space is typed straight after the word', () => {
    // The flicker this guards against: typing the space that finishes a word took its
    // underline away until the next check put it back.
    const before = 'Fix teh';
    expect(carry([mark(before, 'teh')], before, 'Fix teh ')).toEqual([mark(before, 'teh')]);
  });

  it('counts in UTF-16 units, past an emoji', () => {
    const before = '🎉 teh';
    const moved = carry([mark(before, 'teh')], before, '🎉🎉 teh');
    expect(moved).toEqual([{ word: 'teh', start: 5, end: 8 }]);
  });

  it('returns the same marks when nothing changed', () => {
    expect(carry(marks, text, text)).toBe(marks);
  });
});

describe('checking as the message changes', () => {
  it('checks once typing pauses, not once per keystroke', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    for (const typed of ['F', 'Fi', 'Fix', 'Fix ', 'Fix t', 'Fix te', 'Fix teh']) {
      spelling.update(typed, '');
      await vi.advanceTimersByTimeAsync(PAUSE_MS / 5);
    }
    expect(checks()).toBe(0);
    await pause();
    expect(checks()).toBe(1);
    expect(invoke).toHaveBeenCalledWith('spelling_check', { summary: 'Fix teh', body: '', dialect: 'en-US' });
    expect(spelling.shown('summary')).toEqual([{ word: 'teh', start: 4, end: 7 }]);
  });

  it('checks both fields, each counted from its own start', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh', 'It recieve events');
    await pause();
    expect(spelling.shown('summary').map((m) => m.start)).toEqual([4]);
    expect(spelling.shown('body')).toEqual([{ word: 'recieve', start: 3, end: 10 }]);
  });

  it('carries an answer forward to text typed while it was on its way', async () => {
    let answer: (value: unknown) => void = () => undefined;
    invoke.mockImplementation(() => new Promise((resolve) => (answer = resolve)));
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh', '');
    await pause();
    spelling.update('Now fix teh', '');
    answer({ summary: [{ word: 'teh', start: 4, end: 7 }], body: [] });
    await vi.advanceTimersByTimeAsync(0);
    expect(spelling.shown('summary')).toEqual([{ word: 'teh', start: 8, end: 11 }]);
  });

  it('drops an answer once a newer check has started', async () => {
    const answers: ((value: unknown) => void)[] = [];
    invoke.mockImplementation(() => new Promise((resolve) => answers.push(resolve)));
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh', '');
    await pause();
    spelling.update('Fix the', '');
    await pause();
    expect(answers).toHaveLength(2);
    answers[1]?.({ summary: [], body: [] });
    answers[0]?.({ summary: [{ word: 'teh', start: 4, end: 7 }], body: [] });
    await vi.advanceTimersByTimeAsync(0);
    expect(spelling.shown('summary')).toEqual([]);
  });

  it('waits for an input method to finish a character', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.composing(true);
    spelling.update('Fix teh', '');
    await pause();
    expect(checks()).toBe(0);
    spelling.composing(false);
    await pause();
    expect(checks()).toBe(1);
  });

  it('leaves the old underline alone when a check fails', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh', '');
    await pause();
    invoke.mockRejectedValue(new Error('the engine went away'));
    spelling.update('Fix teh now', '');
    await pause();
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh']);
  });
});

describe('the word still being typed', () => {
  it('is not underlined while the caret is at its end', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh', '');
    spelling.typed('summary', 7);
    await pause();
    expect(spelling.shown('summary')).toEqual([]);

    // A space finishes it.
    spelling.update('Fix teh ', '');
    spelling.typed('summary', 8);
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh']);
  });

  it('is underlined once the caret moves away or the field is left', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh', '');
    spelling.typed('summary', 7);
    await pause();
    // A click in the same place is not a move.
    spelling.moved('summary', 7);
    expect(spelling.shown('summary')).toEqual([]);
    spelling.moved('summary', 2);
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh']);

    spelling.typed('summary', 7);
    spelling.left();
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh']);
  });

  it('holds back only in the field being typed in', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh', 'teh');
    spelling.typed('body', 3);
    await pause();
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh']);
    expect(spelling.shown('body')).toEqual([]);
  });
});

describe('what the user says about a word', () => {
  it('ignores a word everywhere for the rest of the session, whatever its case', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Teh teh', 'teh');
    await pause();
    spelling.ignore('teh');
    expect(spelling.shown('summary')).toEqual([]);
    expect(spelling.shown('body')).toEqual([]);
  });

  it('checks again straight away once a word is added', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('Fix teh colour', '');
    await pause();
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh', 'colour']);
    await spelling.learn('colour');
    expect(invoke).toHaveBeenCalledWith('spelling_learn', { word: 'colour' });
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh']);
  });

  it('says so when a word cannot be added, and keeps its underline', async () => {
    engine();
    const errors: string[] = [];
    const spelling = new SpellingState(new ViewsState(), (message) => errors.push(message));
    spelling.update('Fix teh', '');
    await pause();
    invoke.mockRejectedValue({ code: 'io', message: 'permission denied' });
    await spelling.learn('teh');
    expect(errors).toEqual(['permission denied']);
    expect(spelling.shown('summary').map((m) => m.word)).toEqual(['teh']);
  });

  it('asks for suggestions in the chosen dialect, and offers none when that fails', async () => {
    engine();
    const views = new ViewsState();
    views.set('spelling', 'en-GB');
    const spelling = new SpellingState(views);
    expect(await spelling.suggest('teh')).toEqual(['the', 'eh']);
    expect(invoke).toHaveBeenCalledWith('spelling_suggest', { word: 'teh', dialect: 'en-GB' });
    invoke.mockRejectedValue(new Error('gone'));
    expect(await spelling.suggest('teh')).toEqual([]);
  });
});

describe('an empty message', () => {
  it('is not sent to be checked', async () => {
    engine();
    const spelling = new SpellingState(new ViewsState());
    spelling.update('', '');
    await pause();
    expect(checks()).toBe(0);
    spelling.update('Fix teh', '');
    await pause();
    spelling.update('', '');
    expect(spelling.shown('summary')).toEqual([]);
    await pause();
    expect(checks()).toBe(1);
  });
});

describe('the dialect', () => {
  it('asks nothing of the engine and underlines nothing when checking is off', async () => {
    engine();
    const views = new ViewsState();
    const spelling = new SpellingState(views);
    spelling.update('Fix teh', '');
    await pause();
    expect(spelling.shown('summary')).toHaveLength(1);

    views.set('spelling', 'off');
    spelling.update('Fix teh', '');
    expect(spelling.shown('summary')).toEqual([]);
    invoke.mockClear();
    spelling.update('Fix teh more', '');
    await pause();
    expect(invoke).not.toHaveBeenCalled();
    expect(await spelling.suggest('teh')).toEqual([]);
  });

  it('checks again in the new dialect, dropping the old one’s marks at once', async () => {
    engine(['colour']);
    const views = new ViewsState();
    const spelling = new SpellingState(views);
    spelling.update('Pick a colour', '');
    await pause();
    expect(spelling.shown('summary')).toHaveLength(1);

    engine(['color']);
    views.set('spelling', 'en-GB');
    spelling.update('Pick a colour', '');
    expect(spelling.shown('summary')).toEqual([]);
    await pause();
    expect(invoke).toHaveBeenLastCalledWith('spelling_check', { summary: 'Pick a colour', body: '', dialect: 'en-GB' });
  });
});
