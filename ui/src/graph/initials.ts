/**
 * Author initials for a commit node.
 *
 * Initials rather than a fetched avatar: the graph draws a node for every visible row, and a
 * Gravatar lookup per author would put a network round trip on the scroll path and leak the
 * committer's email hash to a third party for a repository the user may not want to announce.
 */

/** Up to two letters for `name`, or `null` when there is nothing to draw. */
export function initialsOf(name: string): string | null {
  const words = name.trim().split(/[\s_.-]+/u).filter(Boolean);
  const first = words[0];
  if (first === undefined) return null;

  const last = words.length > 1 ? words[words.length - 1] : undefined;
  const head = firstLetter(first);
  if (head === null) return null;
  const tail = last === undefined ? null : firstLetter(last);
  return tail === null ? head : head + tail;
}

/**
 * The first letter of a word, uppercased.
 *
 * Iterating the string yields whole code points, so a name starting outside the BMP gives one
 * character rather than half a surrogate pair. Scripts without case are returned unchanged.
 */
function firstLetter(word: string): string | null {
  for (const ch of word) {
    if (/\p{L}|\p{N}/u.test(ch)) return ch.toLocaleUpperCase();
  }
  return null;
}

/**
 * A stable colour for an author, as an index into the node palette.
 *
 * The same person is the same colour in every repository and across restarts, which is what
 * makes a wall of nodes readable at all — the eye picks up "this run is all one person"
 * without reading a single name. Hashed from the identity rather than assigned in order of
 * appearance, or scrolling would renumber everyone.
 */
export function authorColourIndex(identity: string, palette: number): number {
  if (palette <= 0) return 0;
  // FNV-1a: a few lines, no dependency, and well enough distributed for a dozen buckets.
  let hash = 0x811c9dc5;
  for (let i = 0; i < identity.length; i++) {
    hash ^= identity.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash % palette;
}
