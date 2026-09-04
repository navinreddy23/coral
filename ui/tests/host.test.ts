import { describe, expect, it } from 'vitest';

import { hostOf } from '../src/app/HostMark.svelte';

describe('recognising a host from a remote URL', () => {
  it('reads the host out of every form git accepts', () => {
    for (const url of [
      'https://github.com/torvalds/linux.git',
      'http://github.com/torvalds/linux',
      'git@github.com:torvalds/linux.git',
      'ssh://git@github.com/torvalds/linux.git',
      'git://github.com/torvalds/linux.git',
    ]) {
      expect(hostOf(url), url).toBe('github');
    }
  });

  it('recognises a self-hosted instance by its hostname', () => {
    expect(hostOf('https://gitlab.example.com/team/app.git')).toBe('gitlab');
    expect(hostOf('git@github.acme.corp:team/app.git')).toBe('github');
  });

  it('gives anything else the cloud rather than a guess', () => {
    // The remote works perfectly well; it is simply not one we have a picture of.
    for (const url of [
      'https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git',
      'https://bitbucket.org/owner/repo.git',
      'git@internal-server:team/app.git',
      '/srv/git/bare.git',
      '',
    ]) {
      expect(hostOf(url), url).toBe('other');
    }
  });

  it('is not fooled by the name appearing later in the URL', () => {
    // The path is not the host: a repository called `github` on someone's own server is theirs.
    expect(hostOf('https://git.example.com/github/mirror.git')).toBe('other');
    expect(hostOf('git@example.com:gitlab/notes.git')).toBe('other');
  });
});
