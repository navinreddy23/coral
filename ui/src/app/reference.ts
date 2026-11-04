import type { Host } from '../ipc/commands';

/** A run of a commit message: plain text, or something it refers to. */
export type Segment =
  | { kind: 'text'; text: string }
  | { kind: 'issue'; text: string; number: number }
  | { kind: 'request'; text: string; number: number }
  | { kind: 'key'; text: string; key: string };

/**
 * How each host writes a reference. GitLab keeps `#` for issues and `!` for merge requests;
 * Azure DevOps writes a work item as `AB#12` (and `#12` from its own repositories) and a pull
 * request as `!12`; the rest write both as `#12`, and GitHub's issue page sends a pull request
 * on to its own.
 */
function patternFor(kind: Host['kind'] | null): RegExp {
  // Not after a word character or `&`, so `abc#1`, an HTML entity and a URL fragment stay text.
  const ref = kind === 'azure' ? String.raw`(?:AB)?#\d+|!\d+` : kind === 'gitlab' ? String.raw`#\d+|!\d+` : String.raw`#\d+`;
  return new RegExp(String.raw`(?<![\w&/])(?:${ref})\b|\b[A-Z][A-Z0-9]+-\d+\b`, 'gu');
}

/**
 * Cuts a message into text and references. `KEY-123` is a reference only where a tracker
 * is set for it, and nothing is one where there is no host to look it up on.
 */
export function segments(
  text: string,
  kind: Host['kind'] | null,
  tracker: string | null,
): Segment[] {
  const out: Segment[] = [];
  let at = 0;
  const push = (s: Segment) => {
    const last = out.at(-1);
    if (s.kind === 'text' && last?.kind === 'text') last.text += s.text;
    else out.push(s);
  };
  for (const m of text.matchAll(patternFor(kind))) {
    const found = m[0];
    const index = m.index ?? 0;
    let made: Segment | null = null;
    if (/^[A-Z]/u.test(found) && !found.startsWith('AB#')) {
      if (tracker !== null) made = { kind: 'key', text: found, key: found };
    } else if (kind !== null) {
      const number = Number(found.replace(/^\D+/u, ''));
      made = found.startsWith('!')
        ? { kind: 'request', text: found, number }
        : { kind: 'issue', text: found, number };
    }
    if (made === null) continue;
    if (index > at) push({ kind: 'text', text: text.slice(at, index) });
    push(made);
    at = index + found.length;
  }
  if (at < text.length) push({ kind: 'text', text: text.slice(at) });
  return out;
}

/** The issues a message names, each once, in the order it names them. */
export function mentioned(text: string, kind: Host['kind'] | null): number[] {
  const seen = new Set<number>();
  for (const s of segments(text, kind, null)) if (s.kind === 'issue') seen.add(s.number);
  return [...seen];
}

/** Where a reference opens. */
export function linkOf(segment: Segment, host: Host | null, tracker: string | null): string | null {
  if (segment.kind === 'key') return tracker?.replace('{key}', encodeURIComponent(segment.key)) ?? null;
  if (segment.kind === 'text' || host === null) return null;
  const { origin, owner, repo } = host;
  const n = segment.number;
  if (segment.kind === 'request') {
    return host.kind === 'azure'
      ? `${origin}/${owner}/_git/${repo}/pullrequest/${n}`
      : `${origin}/${owner}/${repo}/-/merge_requests/${n}`;
  }
  switch (host.kind) {
    case 'gitlab':
      return `${origin}/${owner}/${repo}/-/issues/${n}`;
    case 'azure':
      return `${origin}/${owner}/_workitems/edit/${n}`;
    default:
      return `${origin}/${owner}/${repo}/issues/${n}`;
  }
}
