import { describe, expect, it } from 'vitest';

import { ignoreChoices } from '../src/app/ignoring';

describe('ways to ignore an untracked path', () => {
  it('offers the file and its extension, narrowest first, and never its folder', () => {
    // src/ may hold tracked files, and /src/ would silently ignore every new one beside them.
    expect(ignoreChoices('src/debug.log').map((c) => [c.label, c.hint])).toEqual([
      ['This file', '/src/debug.log'],
      ['Every .log file', '*.log'],
    ]);
  });

  it('leaves out an extension a file does not have', () => {
    expect(ignoreChoices('Makefile.local').map((c) => c.hint)).toEqual([
      '/Makefile.local',
      '*.local',
    ]);
    expect(ignoreChoices('README').map((c) => c.hint)).toEqual(['/README']);
  });

  it('does not treat a dot file\'s name as an extension', () => {
    expect(ignoreChoices('.envrc').map((c) => c.hint)).toEqual(['/.envrc']);
  });

  it('offers a wholly untracked folder as itself', () => {
    expect(ignoreChoices('dist/')).toEqual([
      { label: 'This folder', hint: '/dist/', what: { kind: 'file', file: 'dist/' } },
    ]);
  });

  it('offers no pattern it would have to escape', () => {
    expect(ignoreChoices('notes.v[2]').map((c) => c.label)).toEqual(['This file']);
  });
});
