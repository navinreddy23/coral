import { describe, expect, it } from 'vitest';

import { splitPrefix } from '../src/app/subject';

describe('splitPrefix', () => {
  it('takes a Conventional Commits type off the front', () => {
    expect(splitPrefix('feat: add a dark theme')).toEqual({ prefix: 'feat:', rest: 'add a dark theme' });
  });

  it('keeps a scope and a breaking-change mark with the type', () => {
    expect(splitPrefix('fix(parser)!: drop the old syntax')).toEqual({
      prefix: 'fix(parser)!:',
      rest: 'drop the old syntax',
    });
  });

  it('takes the subsystem off a kernel-style subject, and only the first one', () => {
    expect(splitPrefix('smb/client: fix stale page cache')).toEqual({
      prefix: 'smb/client:',
      rest: 'fix stale page cache',
    });
    expect(splitPrefix('smb: client: reject SetEA requests')).toEqual({
      prefix: 'smb:',
      rest: 'client: reject SetEA requests',
    });
  });

  it('leaves a subject alone that has no prefix', () => {
    expect(splitPrefix("Merge branch 'feature/parser'")).toBeNull();
    expect(splitPrefix('Fix the build on older compilers')).toBeNull();
    expect(splitPrefix('Revert "ui: tidy the palette"')).toBeNull();
  });

  it('does not take a sentence that happens to contain a colon', () => {
    // A word and a space before the colon is prose, not a prefix.
    expect(splitPrefix('Note to self: tidy this')).toBeNull();
    // Nor a time of day, or a URL.
    expect(splitPrefix('12:30 standup notes')).toBeNull();
    expect(splitPrefix('https://example.com is down')).toBeNull();
  });

  it('leaves a prefix with nothing after it, or nothing but the prefix, alone', () => {
    expect(splitPrefix('wip:')).toBeNull();
    expect(splitPrefix('wip:   ')).toBeNull();
  });

  it('does not take something too long to be a type or a subsystem', () => {
    expect(splitPrefix(`${'a'.repeat(40)}: the rest`)).toBeNull();
  });
});
