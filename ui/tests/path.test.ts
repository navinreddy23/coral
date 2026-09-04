import { describe, expect, it } from 'vitest';

import { elidePath } from '../src/app/path';

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
