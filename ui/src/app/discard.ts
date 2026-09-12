import type { Choice } from './Ask.svelte';

/**
 * What throwing away working-tree changes is called, and what it promises.
 *
 * The two halves are not the same promise, so the dialog splits them rather than the engine
 * doing it: a tracked file goes back to what HEAD holds and its content is still in the object
 * database; a file git has never seen is deleted from disk and exists nowhere else. Every
 * number and every plural in that sentence is load-bearing, which is why this is here rather
 * than spelled out at the call.
 */

/** "1 change", "4 changes" — the plural of a count, without a library for it. */
export function count(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? '' : 's'}`;
}

/**
 * The buttons and the explanation for discarding.
 *
 * Nothing here is primary, so Enter dismisses rather than discarding, and deleting the files
 * git has never seen is always a button of its own.
 */
export function discardWords(
  tracked: number,
  untracked: number,
  /** The branch HEAD is on, or null while it is detached. */
  branch: string | null,
): { choices: Choice[]; detail: string } {
  const choices: Choice[] = [];
  if (tracked > 0) {
    choices.push({
      id: 'tracked',
      // Both answers destroy work; the dialog marks them the way the menus mark the lines
      // that open it.
      danger: true,
      label:
        untracked > 0
          ? `Discard ${count(tracked, 'change')}, keep the new files`
          : `Discard ${count(tracked, 'change')}`,
    });
  }
  if (untracked > 0) {
    choices.push({
      id: 'all',
      danger: true,
      label:
        tracked > 0
          ? `Discard everything, deleting ${count(untracked, 'new file')}`
          : `Delete ${count(untracked, 'new file')}`,
    });
  }
  return { choices, detail: detailOf(tracked, untracked, branch) };
}

function detailOf(tracked: number, untracked: number, branch: string | null): string {
  const where = branch === null ? 'the commit that is checked out' : branch;
  const parts: string[] = [];
  if (tracked > 0) {
    const goes = tracked === 1 ? 'goes' : 'go';
    parts.push(`${count(tracked, 'file')} ${goes} back to what ${where} last committed.`);
  }
  if (untracked > 0) {
    parts.push(
      `${count(untracked, 'file')} ${untracked === 1 ? 'is' : 'are'} not tracked by git,` +
        ` so deleting ${untracked === 1 ? 'it' : 'them'} removes the only copy there is.`,
    );
  }
  parts.push('This cannot be undone.');
  return parts.join(' ');
}
