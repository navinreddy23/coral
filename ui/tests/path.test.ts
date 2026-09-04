import { describe, expect, it } from 'vitest';

import { elidePath, elideRef } from '../src/app/path';

describe('elidePath', () => {
  it('leaves a path that fits alone', () => {
    expect(elidePath('/home/dev/coral', 40)).toBe('/home/dev/coral');
  });

  it('keeps the leading slash where it belongs', () => {
    // `direction: rtl` draws `/home/x` as `home/x/`, which is the whole reason this exists.
    expect(elidePath('/home/dev/coral', 40).startsWith('/')).toBe(true);
  });

  it('drops whole segments from the front', () => {
    // Half a directory name identifies nothing.
    expect(elidePath('/home/dev/projects/coral/ui/src', 20)).toBe('…/coral/ui/src');
  });

  it('always keeps the last segment, which is what names the thing', () => {
    expect(elidePath('/a/b/c/d/e/file.rs', 12)).toContain('file.rs');
  });

  it('cuts into a last segment too long to keep whole', () => {
    const out = elidePath('/some/dir/an-extremely-long-file-name.rs', 14);
    expect(out.length).toBeLessThanOrEqual(14);
    expect(out.endsWith('name.rs')).toBe(true);
  });

  it('never returns more than it was asked for', () => {
    const path = '/home/dev/projects/coral/crates/coral-core/src/graph/lanes.rs';
    for (const max of [8, 12, 20, 30, 50]) {
      expect(elidePath(path, max).length, `max ${max}`).toBeLessThanOrEqual(max);
    }
  });

  it('handles a relative path and a bare name', () => {
    expect(elidePath('ui/src/app/App.svelte', 60)).toBe('ui/src/app/App.svelte');
    // A bare name with no room keeps its end, which is where a suffix lives.
    expect(elidePath('README', 3)).toBe('…ME');
  });
});

describe('elideRef', () => {
  it('leaves a name that fits alone', () => {
    expect(elideRef('main', 20)).toBe('main');
    expect(elideRef('origin/main', 20)).toBe('origin/main');
  });

  it('keeps the branch and the remote, dropping what is between them', () => {
    // Cut from the right, `origin/feature/audio-ringbuffer` becomes `origin/feature/audio-r…`,
    // which is every branch on that feature. This keeps the half that names exactly one.
    const short = elideRef('origin/feature/audio-ringbuffer', 26);
    expect(short).toBe('origin/…/audio-ringbuffer');
  });

  it('gives up the remote before it gives up the branch', () => {
    // One character narrower than the pair needs. The branch is what identifies the ref, so
    // it is the half that survives.
    expect(elideRef('origin/feature/audio-ringbuffer', 24)).toBe('…/audio-ringbuffer');
  });

  it('drops the remote too when even that will not fit', () => {
    const short = elideRef('someverylongremote/feature/audio-ringbuffer', 20);
    expect(short).toBe('…/audio-ringbuffer');
    expect(short.length).toBeLessThanOrEqual(20);
  });

  it('cuts a single long segment from the right, since it has no parts to drop', () => {
    const short = elideRef('averyveryverylongbranchnamewithnoslashes', 12);
    expect(short.length).toBeLessThanOrEqual(12);
    expect(short.endsWith('…')).toBe(true);
  });

  it('never returns more than it was asked for', () => {
    for (const name of [
      'origin/main',
      'origin/feature/a/b/c/d/e/audio',
      'v1.24.0-preview',
      'refs/remotes/upstream/release/2026.1',
    ]) {
      for (const max of [6, 10, 18, 24]) {
        expect(elideRef(name, max).length).toBeLessThanOrEqual(max);
      }
    }
  });
});

describe('a branch name whose own last segment is too long', () => {
  it('keeps the start of that segment, not the shared prefix', () => {
    // Cut from the right, `origin/bugfix/REAN2-6063-discard-triplog` reads
    // `origin/bugfix/REAN2-6…`, which every branch on that ticket shares. What identifies
    // one branch is the start of its own last segment.
    const short = elideRef('origin/bugfix/REAN2-6063-discard-unhandlable-triplog', 26);
    expect(short).toBe('…/REAN2-6063-discard-unha…');
    expect(short.length).toBeLessThanOrEqual(26);
  });

  it('still fits whatever it is given', () => {
    for (const max of [6, 8, 12, 22, 26, 40]) {
      const short = elideRef('origin/maintenance/code-savings-ddiv-dmul-removal', max);
      expect(short.length, `max ${max}: ${short}`).toBeLessThanOrEqual(max);
    }
  });
});
