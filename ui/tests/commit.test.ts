import { describe, expect, it } from 'vitest';
import { CommitState } from '../src/state/commit.svelte';

/** Enough of a worktree for `ready`: one staged file and nothing running. */
const staged = { staged: [{ path: 'a.txt' }], busy: false } as never;

describe('the message being written', () => {
  it('joins the summary and the description the way git expects', () => {
    const draft = new CommitState();
    draft.summary = '  a summary  ';
    expect(draft.message).toBe('a summary');
    draft.description = '  and why  ';
    expect(draft.message).toBe('a summary\n\nand why');
  });

  it('needs something staged to commit, but not to amend', () => {
    const draft = new CommitState();
    draft.summary = 'something';
    const empty = { staged: [], busy: false } as never;
    expect(draft.ready(empty)).toBe(false);
    draft.amend = true;
    expect(draft.ready(empty)).toBe(true);
    expect(draft.ready(staged)).toBe(true);
  });
});

describe('seeding an amend', () => {
  it('fills both fields from the commit being replaced', () => {
    const draft = new CommitState();
    draft.seed('fix: the summary', 'and the body\n');
    expect(draft.summary).toBe('fix: the summary');
    expect(draft.description).toBe('and the body');
    expect(draft.message).toBe('fix: the summary\n\nand the body');
  });

  /** Unticking has to leave the box as it found it, or it eats a message it did not write. */
  it('takes back what it put there, and only that', () => {
    const draft = new CommitState();
    draft.seed('fix: the summary', '');
    draft.unseed();
    expect(draft.summary).toBe('');

    draft.seed('fix: the summary', '');
    draft.summary = 'fix: what the user typed instead';
    draft.unseed();
    expect(draft.summary).toBe('fix: what the user typed instead');
  });

  it('forgets the seed once the commit is made, so the next untick takes nothing', () => {
    const draft = new CommitState();
    draft.seed('fix: the summary', '');
    draft.clear();
    draft.summary = 'a new commit';
    draft.unseed();
    expect(draft.summary).toBe('a new commit');
  });
});
