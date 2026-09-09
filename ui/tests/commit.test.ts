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

describe('the message git prepared', () => {
  /**
   * A merge, cherry-pick or revert asked for without committing leaves the changes staged and
   * writes the message it would have used into the git dir. Coral opens no editor, so the
   * subject git had already chosen went nowhere and had to be typed again from the row above.
   */
  it('fills the box as a subject and a body', () => {
    const draft = new CommitState();
    draft.prepare("Merge branch 'side'\n\nwhy it was merged");
    expect(draft.summary).toBe("Merge branch 'side'");
    expect(draft.description).toBe('why it was merged');
    expect(draft.message).toBe("Merge branch 'side'\n\nwhy it was merged");
  });

  it('takes a message that is only a subject', () => {
    const draft = new CommitState();
    draft.prepare('Oops: a stray debug line');
    expect(draft.summary).toBe('Oops: a stray debug line');
    expect(draft.description).toBe('');
  });

  /**
   * Unlike the message an amend is seeded with, this one is not taken back. Amend puts a
   * message there because a tick was pressed and removes it when that tick is undone; this is
   * a starting point for a commit that is going to be made either way.
   */
  it('stays when an amend is ticked and unticked over it', () => {
    const draft = new CommitState();
    draft.prepare('Oops: a stray debug line');
    draft.amend = true;
    draft.unseed();
    expect(draft.summary).toBe('Oops: a stray debug line');
  });
});

describe('taking a prepared message back', () => {
  /**
   * A merge finished from the conflict tool never passes through the commit panel, so nothing
   * cleared the draft: the box was left holding "Merge branch 'side'" over a commit that had
   * already been made, ready to be suggested for whatever was staged next.
   */
  it('empties the box when the operation it belonged to is over', () => {
    const draft = new CommitState();
    draft.prepare("Merge branch 'side'");
    draft.withdraw();
    expect(draft.summary).toBe('');
    expect(draft.description).toBe('');
  });

  it('leaves alone what was typed over it', () => {
    const draft = new CommitState();
    draft.prepare("Merge branch 'side'");
    draft.summary = 'Bring the tea page in from side';
    draft.withdraw();
    expect(draft.summary).toBe('Bring the tea page in from side');
  });

  it('takes nothing back twice, nor anything it did not put there', () => {
    const draft = new CommitState();
    draft.prepare('Oops: a stray debug line');
    draft.withdraw();
    draft.summary = 'Something else entirely';
    draft.withdraw();
    expect(draft.summary).toBe('Something else entirely');
  });
});
