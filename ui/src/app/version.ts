/**
 * Ordering for names that carry a version, which is what tags almost always are.
 *
 * Lexicographic order is wrong for them in a way that is immediately obvious on a real
 * repository: the kernel's tag list opens on `v2.6.11` and the release anyone wants is nine
 * hundred rows below it. It is also wrong within a series, since `v7.9` sorts after `v7.10`.
 *
 * This is git's own `version:refname` ordering, reproduced rather than asked for: the refs come
 * from one `for-each-ref` covering branches and tags together, and sorting that by version
 * would reorder the branches too.
 */

function isDigit(ch: string): boolean {
  return ch >= '0' && ch <= '9';
}

/** The digit run starting at `at`, and where it ends. */
function number(name: string, at: number): { value: number; end: number } {
  let end = at;
  while (end < name.length && isDigit(name[end] ?? '')) end += 1;
  return { value: Number(name.slice(at, end)), end };
}

/**
 * Compares two version-ish names, oldest first.
 *
 * Character by character, dropping into a numeric comparison only where both sides have a digit
 * run. Comparing whole runs of non-digits as units looks equivalent and is not: against
 * `v7.3-rc1`, a name like `v-old` would then be compared as the token `v-old` against the token
 * `v`, come out longer and therefore greater, and sort above every release in the repository.
 * Character by character, `-` is simply below `7` and it lands where git puts it.
 *
 * A name that continues where another stops sorts after it, so `v7.2-rc1` comes after `v7.2`,
 * and descending therefore puts the release candidates above the release they lead to. That is
 * what git does without a `versionsort.suffix` configured, and matching it matters more than
 * any opinion about where a candidate belongs.
 */
export function compareVersions(a: string, b: string): number {
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    const x = a[i] ?? '';
    const y = b[j] ?? '';
    if (isDigit(x) && isDigit(y)) {
      const left = number(a, i);
      const right = number(b, j);
      if (left.value !== right.value) return left.value - right.value;
      i = left.end;
      j = right.end;
      continue;
    }
    if (x !== y) return x < y ? -1 : 1;
    i += 1;
    j += 1;
  }
  return a.length - i - (b.length - j);
}

/** The same list, newest first. */
export function byVersionDescending<T>(items: readonly T[], name: (item: T) => string): T[] {
  return [...items].sort((a, b) => compareVersions(name(b), name(a)));
}
