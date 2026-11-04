/**
 * The folders a cone needs to be told about, from the ones ticked.
 *
 * A folder in a cone brings everything beneath it, so a ticked folder inside another ticked
 * one says nothing more and is left out, and the rest are sorted the way git lists them.
 */
export function coneOf(ticked: Iterable<string>): string[] {
  const all = [...new Set([...ticked].map((f) => f.replace(/\/+$/u, '')).filter((f) => f !== ''))].sort();
  return all.filter((f) => !all.some((other) => other !== f && f.startsWith(`${other}/`)));
}

/** Whether `folder` is in the cone because a folder above it was ticked. */
export function coveredBy(folder: string, ticked: Iterable<string>): boolean {
  for (const t of ticked) if (folder.startsWith(`${t}/`)) return true;
  return false;
}
