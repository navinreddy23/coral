/**
 * Shortens a path from the left, keeping the end.
 *
 * Done here rather than with `direction: rtl` and `text-overflow`, which is the usual trick
 * and is wrong for a path: the bidirectional algorithm treats the leading `/` as neutral and
 * reorders it to the far end, so `/home/dev/projects/coral` is drawn as
 * `home/dev/projects/coral/`. Nothing about that is visible until someone reads the path.
 *
 * Whole segments are dropped where they fit, because half a directory name identifies nothing.
 */
export function elidePath(path: string, max: number): string {
  if (path.length <= max) return path;

  const segments = path.split('/');
  const name = segments.pop() ?? path;

  // The last segment alone may already be too long; cut it rather than return something wider
  // than asked for, and keep the end, which is the part that names the thing.
  if (name.length + 2 > max) return `…${name.slice(name.length - Math.max(1, max - 1))}`;

  let kept = name;
  for (let i = segments.length - 1; i >= 0; i--) {
    const next = `${segments[i]}/${kept}`;
    if (next.length + 2 > max) break;
    kept = next;
  }
  return `…/${kept}`;
}
