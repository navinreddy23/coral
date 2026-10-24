/** What can be added to `.gitignore`: one path exactly, or a pattern as written. */
export type Ignored = { kind: 'file'; file: string } | { kind: 'pattern'; pattern: string };

export interface IgnoreChoice {
  label: string;
  /** The line it adds, so nobody has to guess what "every log file" will match. */
  hint: string;
  what: Ignored;
}

/**
 * The ways an untracked path can be ignored, narrowest first.
 *
 * git lists a wholly untracked folder as one entry ending in a slash, so a folder is offered
 * as itself. A file's own folder is not offered: when anything in it is tracked, `/src/`
 * would leave those files alone and silently ignore every new one added beside them.
 *
 * An extension is offered only when it is plain letters and digits: anything else would have
 * to be escaped, and a pattern the user cannot read is not one they should be adding to a file
 * they keep.
 */
export function ignoreChoices(path: string): IgnoreChoice[] {
  if (path.endsWith('/')) {
    return [{ label: 'This folder', hint: `/${path}`, what: { kind: 'file', file: path } }];
  }
  const out: IgnoreChoice[] = [
    { label: 'This file', hint: `/${path}`, what: { kind: 'file', file: path } },
  ];
  const slash = path.lastIndexOf('/');
  const name = path.slice(slash + 1);
  const dot = name.lastIndexOf('.');
  const extension = dot > 0 ? name.slice(dot + 1) : '';
  if (/^[A-Za-z0-9_-]+$/.test(extension)) {
    out.push({
      label: `Every .${extension} file`,
      hint: `*.${extension}`,
      what: { kind: 'pattern', pattern: `*.${extension}` },
    });
  }
  return out;
}
