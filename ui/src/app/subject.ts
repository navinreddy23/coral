/**
 * The prefix a commit subject leads with, split off so the list can set it quieter than the
 * sentence after it: a Conventional Commits type (`fix(parser)!:`) or a kernel-style
 * subsystem (`smb/client:`).
 *
 * One word with no spaces, then a colon and a space. That is what tells `ui: tidy` from
 * `Note to self: tidy`, `12:30 standup` and a URL, none of which are prefixes.
 */
const PREFIX = /^([A-Za-z0-9][\w./-]{0,23}(?:\([^()]{1,32}\))?!?:)\s+(\S.*)$/u;

export function splitPrefix(subject: string): { prefix: string; rest: string } | null {
  const found = PREFIX.exec(subject);
  if (!found?.[1] || !found[2]) return null;
  return { prefix: found[1], rest: found[2] };
}
