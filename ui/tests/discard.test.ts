import { describe, expect, it } from 'vitest';

import { count, discardWords } from '../src/app/discard';

const labels = (n: number, u: number, branch: string | null = 'master') =>
  discardWords(n, u, branch).choices.map((c) => c.label);

const detail = (n: number, u: number, branch: string | null = 'master') =>
  discardWords(n, u, branch).detail;

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
    expect(detail(0, 1)).toContain('1 file is not tracked');
    expect(detail(0, 2)).toContain('2 files are not tracked');
  });

  it('says that the untracked files exist nowhere else', () => {
    expect(detail(0, 3)).toContain('the only copy there is');
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
      const { choices } = discardWords(tracked, untracked, 'main');
      expect(choices.length).toBeGreaterThan(0);
      expect(choices.every((c) => c.danger === true), `${tracked}/${untracked}`).toBe(true);
    }
  });
});
