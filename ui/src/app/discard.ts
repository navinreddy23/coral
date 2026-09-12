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

/** The same for a noun whose plural is not the singular with an s on it. */
function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/**
 * What deleting the untracked entries actually takes.
 *
 * git lists a directory nobody has added as one entry — `? build/` — rather than walking it,
 * which is what keeps `status` fast on a repository with a build tree in it. Repeating that
 * count as a number of files made the dialog undercount: "Delete 2 new files" about a button
 * that removed a loose file and a directory holding six, and the dialog's whole job is to say
 * what is going.
 */
export function untrackedWords(paths: readonly string[]): string {
  const dirs = paths.filter((p) => p.endsWith('/')).length;
  const files = paths.length - dirs;
  if (dirs === 0) return count(files, 'new file');
  const both = plural(dirs, 'new directory', 'new directories');
  return files === 0 ? both : `${count(files, 'new file')} and ${both}`;
}

/**
 * The buttons and the explanation for discarding.
 *
 * Nothing here is primary, so Enter dismisses rather than discarding, and deleting the files
 * git has never seen is always a button of its own.
 */
export function discardWords(
  tracked: number,
  /** The untracked paths themselves: a directory among them is one entry and many files. */
  untrackedPaths: readonly string[],
  /** The branch HEAD is on, or null while it is detached. */
  branch: string | null,
): { choices: Choice[]; detail: string } {
  const untracked = untrackedPaths.length;
  const going = untrackedWords(untrackedPaths);
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
      label: tracked > 0 ? `Discard everything, deleting ${going}` : `Delete ${going}`,
    });
  }
  return { choices, detail: detailOf(tracked, untrackedPaths, branch) };
}

function detailOf(
  tracked: number,
  untrackedPaths: readonly string[],
  branch: string | null,
): string {
  const untracked = untrackedPaths.length;
  const where = branch === null ? 'the commit that is checked out' : branch;
  const parts: string[] = [];
  if (tracked > 0) {
    const goes = tracked === 1 ? 'goes' : 'go';
    parts.push(`${count(tracked, 'file')} ${goes} back to what ${where} last committed.`);
  }
  if (untracked > 0) {
    parts.push(
      `${untrackedWords(untrackedPaths)} ${untracked === 1 ? 'is' : 'are'} not tracked by git,` +
        ` so deleting ${untracked === 1 ? 'it' : 'them'} removes the only copy there is.`,
    );
    if (untrackedPaths.some((p) => p.endsWith('/'))) {
      parts.push('A directory goes with everything inside it.');
    }
  }
  parts.push('This cannot be undone.');
  return parts.join(' ');
}
