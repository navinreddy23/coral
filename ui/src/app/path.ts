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

/**
 * Shortens a ref name for a pill, keeping the part that identifies the branch.
 *
 * A ref is a path too, and the same rule applies: `origin/feature/audio-ringbuffer` cut from
 * the right becomes `origin/feature/audio-rin…`, which is every branch on that feature. Cut
 * from the left it becomes `…/audio-ringbuffer`, which is exactly one.
 *
 * The remote's own name is worth keeping when it fits, since it is what separates a tracking
 * branch from the local one beside it.
 */
export function elideRef(name: string, max: number): string {
  if (name.length <= max) return name;

  const parts = name.split('/');
  const last = parts.pop() ?? name;
  // Nothing to drop: one long word, so it can only be cut.
  if (parts.length === 0) return `${name.slice(0, Math.max(1, max - 1))}…`;

  // The remote or the first segment, plus the tail: `origin/…/audio-ringbuffer`.
  const head = parts[0] ?? '';
  const withHead = `${head}/…/${last}`;
  if (withHead.length <= max) return withHead;
  if (last.length + 2 <= max) return `…/${last}`;

  // Even the last segment alone is too long. Cut *it* rather than the whole name: a branch
  // called `bugfix/REAN2-6063-discard-triplog` shortened from the right reads
  // `origin/bugfix/REAN2-6…`, which every branch on that ticket shares. Keeping the segment's
  // own start gives `…/REAN2-6063-discard-…`, which identifies one.
  return `…/${last.slice(0, Math.max(1, max - 3))}…`;
}

/**
 * The directory something sits in, or undefined when there is no directory above it.
 *
 * For a picker that should open *beside* a repository rather than inside it: a new working
 * tree may not be made within the repository it belongs to, and its siblings are where people
 * keep them.
 */
export function beside(path: string | undefined): string | undefined {
  if (path === undefined) return undefined;
  const at = path.replace(/[/\\]+$/u, '').lastIndexOf('/');
  return at > 0 ? path.slice(0, at) : undefined;
}

/**
 * The last part of a path: the folder or file's own name.
 *
 * Either separator ends a part, since a path chosen in a Windows file dialog uses a backslash.
 */
export function baseName(path: string): string {
  const trimmed = path.replace(/[/\\]+$/u, '');
  return trimmed.slice(Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\')) + 1);
}

/**
 * Where `git submodule add` puts a submodule when it is given no path: the URL's last
 * component, without `.git`.
 *
 * Follows `git_url_basename` in git's `dir.c`, so the path the form fills in is the one git
 * would have chosen: the scheme and any `user@` are skipped, trailing slashes and a `/.git`
 * are dropped, and a colon separates components as a slash does, for `host:repo.git`.
 */
export function submodulePath(url: string): string {
  let rest = url.trim();
  const scheme = rest.indexOf('://');
  if (scheme >= 0) rest = rest.slice(scheme + 3);
  const stop = rest.search(/[/\\@]/u);
  if (stop >= 0 && rest[stop] === '@') rest = rest.slice(stop + 1);
  rest = rest.replace(/[/\\\s]+$/u, '').replace(/[/\\]\.git$/u, '').replace(/[/\\]+$/u, '');
  const last = Math.max(rest.lastIndexOf('/'), rest.lastIndexOf('\\'), rest.lastIndexOf(':'));
  return rest.slice(last + 1).replace(/\.git$/u, '');
}
