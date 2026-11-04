import { describe, expect, it } from 'vitest';
import { linkOf, mentioned, segments } from '../src/app/reference';
import type { Host } from '../src/ipc/commands';

const github: Host = { kind: 'github', origin: 'https://github.com', owner: 'o', repo: 'r' };
const gitlab: Host = { kind: 'gitlab', origin: 'https://gitlab.com', owner: 'g', repo: 'p' };
const azure: Host = { kind: 'azure', origin: 'https://dev.azure.com', owner: 'acme/P', repo: 'coral' };

describe('references in a commit message', () => {
  it('finds issues by the way each host writes them, and leaves look-alikes as text', () => {
    const s = segments('Fixes #12 and see #3; not abc#4 nor &#38; nor x.com/#9', 'github', null);
    expect(s.filter((x) => x.kind === 'issue').map((x) => x.text)).toEqual(['#12', '#3']);
    expect(s.map((x) => x.text).join('')).toBe('Fixes #12 and see #3; not abc#4 nor &#38; nor x.com/#9');

    const gl = segments('Follows !7, closes #2', 'gitlab', null);
    expect(gl.map((x) => x.kind)).toEqual(['text', 'request', 'text', 'issue']);
    expect(linkOf(gl[1]!, gitlab, null)).toBe('https://gitlab.com/g/p/-/merge_requests/7');
    expect(linkOf(gl[3]!, gitlab, null)).toBe('https://gitlab.com/g/p/-/issues/2');

    const az = segments('AB#41 and !5', 'azure', null);
    expect(linkOf(az[0]!, azure, null)).toBe('https://dev.azure.com/acme/P/_workitems/edit/41');
    expect(linkOf(az[2]!, azure, null)).toBe('https://dev.azure.com/acme/P/_git/coral/pullrequest/5');
  });

  it('links KEY-123 only through a tracker, and nothing without a host', () => {
    expect(segments('PROJ-12 done', 'github', null).every((x) => x.kind !== 'key')).toBe(true);
    const tracked = segments('PROJ-12 done', 'github', 'https://jira.example.invalid/browse/{key}');
    expect(linkOf(tracked[0]!, github, 'https://jira.example.invalid/browse/{key}')).toBe(
      'https://jira.example.invalid/browse/PROJ-12',
    );
    expect(segments('Fixes #12', null, null)).toEqual([{ kind: 'text', text: 'Fixes #12' }]);
  });

  it('lists each issue a message names once', () => {
    expect(mentioned('#3, #4 and #3 again; !9', 'gitlab')).toEqual([3, 4]);
  });
});
