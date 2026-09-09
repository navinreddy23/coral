import { describe, expect, it } from 'vitest';

import { emptied, plainly } from '../src/state/merge.svelte';

/**
 * A cherry-pick or revert whose change is already present ends up with nothing to record, and
 * git answers with two commands to type. This window has a button for one of them, no way at
 * all to do the other, and no terminal in the way — so the advice named the thing the reader
 * could not do and hid the two they could.
 */
describe('what a stop with nothing to record says', () => {
  const said =
    'The previous cherry-pick is now empty, possibly due to conflict resolution.\n' +
    'If you wish to commit it anyway, use:\n\n    git commit --allow-empty\n\n' +
    "Otherwise, please use 'git cherry-pick --skip'";

  it('names the buttons that are there rather than the commands that are not', () => {
    const answer = plainly(said);
    expect(answer).toContain('nothing left to record');
    expect(answer).toContain('Skip commit');
    expect(answer).toContain('Abort');
    expect(answer).not.toContain('git commit --allow-empty');
    expect(answer).not.toContain('--skip');
  });

  it('is recognised so the rest of the window can answer it too', () => {
    expect(emptied(said)).toBe(true);
    expect(emptied('CONFLICT (content): Merge conflict in log.txt')).toBe(false);
    expect(emptied('')).toBe(false);
  });

  it('leaves every other thing git says exactly as it said it', () => {
    const conflict = 'error: could not apply 1a2b3c4… the commit';
    expect(plainly(conflict)).toBe(conflict);
  });
});

describe('the answer is kept, not read back off the words', () => {
  /**
   * `plainly` rewrites git's message, so asking the result whether it is git's original can
   * only ever say no — which left Continue lit and the summary still saying every file was
   * resolved, under a line explaining that there was nothing to record.
   */
  it('does not recognise its own rewriting', () => {
    const said = 'The previous cherry-pick is now empty, possibly due to conflict resolution.';
    expect(emptied(said)).toBe(true);
    expect(emptied(plainly(said))).toBe(false);
  });
});
