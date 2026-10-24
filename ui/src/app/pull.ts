/**
 * The files git names when uncommitted work is in a pull's way, or null when that is not why
 * it stopped.
 */
export function inTheWay(message: string | undefined): string[] | null {
  const said = message?.match(
    /local changes to the following files would be overwritten[^:]*:([\s\S]*?)(?:Please|Aborting|$)/u,
  );
  if (!said?.[1]) return null;
  const files = said[1]
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l !== '');
  return files.length > 0 ? files : null;
}
