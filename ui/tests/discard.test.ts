import { describe, expect, it } from 'vitest';

import { count, discardWords, partWords, untrackedWords } from '../src/app/discard';

/** `u` untracked files, named the way git lists loose ones. */
const files = (u: number) => Array.from({ length: u }, (_, i) => `new${i}.txt`);

const labels = (n: number, u: number, branch: string | null = 'master') =>
  discardWords(n, files(u), branch).choices.map((c) => c.label);

const detail = (n: number, u: number, branch: string | null = 'master') =>
  discardWords(n, files(u), branch).detail;

describe('counting things in a sentence', () => {
  it('pluralises on the number, and only past one', () => {
    expect(count(0, 'change')).toBe('0 changes');
    expect(count(1, 'change')).toBe('1 change');
    expect(count(2, 'change')).toBe('2 changes');
  });
});

describe('what discarding offers to do', () => {
  it('keeps the files git has never seen out of the ordinary button', () => {
    // A tracked file goes back to what HEAD holds and is still in the object database. An
    // untracked one is deleted from disk and exists nowhere else, so it is never swept up in
    // the same press.
    expect(labels(3, 2)).toEqual([
      'Discard 3 changes, keep the new files',
      'Discard everything, deleting 2 new files',
    ]);
  });

  it('says plainly what it does when there is only one kind', () => {
    expect(labels(4, 0)).toEqual(['Discard 4 changes']);
    expect(labels(0, 1)).toEqual(['Delete 1 new file']);
  });

  it('offers nothing at all when nothing has changed', () => {
    expect(labels(0, 0)).toEqual([]);
  });
});

describe('what discarding says it will cost', () => {
  it('names the branch the files go back to', () => {
    expect(detail(2, 0)).toContain('back to what master last committed');
  });

  it('has something to say with no branch to name', () => {
    // Detached, there is no branch — and "back to what null last committed" is what a missing
    // fallback here would read as.
    expect(detail(2, 0, null)).toContain('back to what the commit that is checked out last committed');
  });

  it('agrees with itself about one file', () => {
    expect(detail(1, 0)).toContain('1 file goes back');
    expect(detail(2, 0)).toContain('2 files go back');
    expect(detail(0, 1)).toContain('1 new file is not tracked');
    expect(detail(0, 2)).toContain('2 new files are not tracked');
  });

  it('says that the untracked files exist nowhere else', () => {
    expect(detail(0, 3)).toContain('the only copy there is');
  });

  it('agrees with itself about how many there are', () => {
    // "1 file is not tracked by git, so deleting them removes the only copy" — the count and
    // the verb were made to agree and the pronoun was not.
    expect(detail(0, 1)).toContain('so deleting it removes');
    expect(detail(0, 1)).not.toContain('deleting them');
    expect(detail(0, 2)).toContain('so deleting them removes');
  });

  it('always ends by saying it cannot be undone', () => {
    for (const [t, u] of [[1, 0], [0, 1], [2, 3], [0, 0]] as const) {
      expect(detail(t, u).endsWith('This cannot be undone.'), `${t}/${u}`).toBe(true);
    }
  });
});

describe('which of the answers destroy something', () => {
  /**
   * Both of them do. The window marks every menu line that destroys something, and the dialog
   * that asks whether to really do it marks its button; this dialog has two buttons and had
   * neither.
   */
  it('marks them, and there is nothing here that does not', () => {
    for (const [tracked, untracked] of [
      [2, 0],
      [0, 2],
      [2, 2],
    ] as const) {
      const { choices } = discardWords(tracked, files(untracked), 'main');
      expect(choices.length).toBeGreaterThan(0);
      expect(choices.every((c) => c.danger === true), `${tracked}/${untracked}`).toBe(true);
    }
  });
});

describe('a directory nobody has added', () => {
  it('is not counted as one file, because it is not one', () => {
    // git lists it as one entry — `? build/` — which is what keeps status fast. Repeating that
    // as a file count made the dialog say "Delete 2 new files" about a button that removed a
    // loose file and a directory holding six.
    expect(untrackedWords(['a.txt', 'b.txt'])).toBe('2 new files');
    expect(untrackedWords(['build/'])).toBe('1 new directory');
    expect(untrackedWords(['build/', 'out/'])).toBe('2 new directories');
    expect(untrackedWords(['a.txt', 'build/'])).toBe('1 new file and 1 new directory');
  });

  it('says what a directory takes with it', () => {
    const said = discardWords(0, ['a.txt', 'build/'], 'main');
    expect(said.detail).toContain('1 new file and 1 new directory');
    expect(said.detail).toContain('everything inside it');
    expect(said.choices[0]?.label).toBe('Delete 1 new file and 1 new directory');
  });

  it('says nothing about directories when there are none', () => {
    expect(discardWords(0, ['a.txt'], 'main').detail).not.toContain('directory');
  });
});

describe('discarding part of a file', () => {
  it('names where the change goes back to, which is the index', () => {
    // A file with something staged goes back to that, not to the commit. Promising the commit
    // said the question would throw away a staged change it does not touch.
    const staged = partWords('crlf.txt', 2, true);
    expect(staged.what).toBe('2 lines');
    expect(staged.detail).toContain('goes back to what is staged for it');
    expect(staged.detail).not.toContain('what is committed');

    const only = partWords('crlf.txt', 2, false);
    expect(only.detail).toContain('goes back to what is committed');
  });

  it('calls a whole hunk a hunk rather than counting its lines', () => {
    expect(partWords('a.txt', 0, false).what).toBe('this hunk');
    expect(partWords('a.txt', 1, false).what).toBe('1 line');
  });

  it('says the change is in no commit either way, because it is not', () => {
    for (const staged of [true, false]) {
      expect(partWords('a.txt', 0, staged).detail).toContain('nothing to bring it back from');
    }
  });
});
