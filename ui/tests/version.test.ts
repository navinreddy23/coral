import { describe, expect, it } from 'vitest';
import { byVersionDescending, compareVersions } from '../src/app/version';

describe('version ordering', () => {
  it('puts a later release after an earlier one', () => {
    expect(compareVersions('v7.2', 'v7.3')).toBeLessThan(0);
    expect(compareVersions('v7.3', 'v7.2')).toBeGreaterThan(0);
  });

  it('compares numbers as numbers, not as text', () => {
    // The reason lexicographic order cannot be used: v7.9 would sort after v7.10.
    expect(compareVersions('v7.9', 'v7.10')).toBeLessThan(0);
    expect(compareVersions('v2.6.9', 'v2.6.11')).toBeLessThan(0);
  });

  it('orders a release candidate the way git does', () => {
    // git's version:refname with no versionsort.suffix configured: a name that continues where
    // another stops sorts after it, so descending shows the candidates above their release.
    expect(compareVersions('v7.2', 'v7.2-rc1')).toBeLessThan(0);
    expect(compareVersions('v7.2-rc1', 'v7.2-rc7')).toBeLessThan(0);
  });

  it('puts a name with no version below the releases, where git puts it', () => {
    // The case that broke the first attempt. Comparing whole runs of non-digits as units made
    // `v-old` a longer token than `v`, therefore greater, and it sorted above every release.
    expect(compareVersions('v-old', 'v7.3-rc1')).toBeLessThan(0);
    expect(compareVersions('v-mid', 'v2.6.11')).toBeLessThan(0);
    expect(byVersionDescending(['v-old', 'v7.3-rc1', 'v2.6.11'], (t) => t)).toEqual([
      'v7.3-rc1',
      'v2.6.11',
      'v-old',
    ]);
  });

  it('is a total order with no equal pairs among distinct names', () => {
    const names = ['v2.6.11', 'v2.6.11-tree', 'v2.6.12', 'v7.2', 'v7.2-rc1', 'v7.10', 'v7.9'];
    for (const a of names) {
      for (const b of names) {
        if (a === b) expect(compareVersions(a, b)).toBe(0);
        else expect(compareVersions(a, b)).not.toBe(0);
      }
    }
  });

  it('sorts the kernel’s own tags newest first', () => {
    // The order `git tag --sort=-version:refname` gives, on names taken from the kernel.
    const tags = ['v2.6.11', 'v2.6.11-tree', 'v2.6.12', 'v6.0', 'v6.0-rc1', 'v7.2', 'v7.3-rc1'];
    expect(byVersionDescending(tags, (t) => t)).toEqual([
      'v7.3-rc1',
      'v7.2',
      'v6.0-rc1',
      'v6.0',
      'v2.6.12',
      'v2.6.11-tree',
      'v2.6.11',
    ]);
  });

  it('leaves the list it was given alone', () => {
    const tags = ['v1', 'v3', 'v2'];
    byVersionDescending(tags, (t) => t);
    expect(tags).toEqual(['v1', 'v3', 'v2']);
  });

  it('survives a tag that carries no number at all', () => {
    expect(() => byVersionDescending(['stable', 'v1.0', 'nightly'], (t) => t)).not.toThrow();
    expect(byVersionDescending(['stable', 'v1.0', 'nightly'], (t) => t)[0]).toBe('v1.0');
  });
});
