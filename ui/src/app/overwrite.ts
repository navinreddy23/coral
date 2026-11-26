/**
 * The files git names when uncommitted work is in the way of a pull or a checkout, or null
 * when that is not why it stopped.
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

/**
 * Whether a pull stopped because git will not rebase over uncommitted work, which it refuses
 * whatever the pull would change, naming no file.
 */
export function refusedToRebase(message: string | undefined): boolean {
  return (
    message?.match(
      /cannot pull with rebase: (?:You have unstaged changes|Your index contains uncommitted changes)/u,
    ) != null
  );
}
