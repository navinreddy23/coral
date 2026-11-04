import { describe, expect, it } from 'vitest';
import { coneOf, coveredBy, holds } from '../src/app/cone';

describe('a cone from ticked folders', () => {
  it('leaves out a folder inside another ticked one, and sorts the rest', () => {
    expect(coneOf(['kernel', 'drivers/gpu/drm', 'drivers/gpu', 'arch/x86/'])).toEqual([
      'arch/x86',
      'drivers/gpu',
      'kernel',
    ]);
    // A shared prefix is not a parent: drivers/gpu2 is not inside drivers/gpu.
    expect(coneOf(['drivers/gpu', 'drivers/gpu2'])).toEqual(['drivers/gpu', 'drivers/gpu2']);
    expect(coneOf([])).toEqual([]);
  });

  it('says which folders are in only because a folder above them is', () => {
    const ticked = new Set(['drivers/gpu']);
    expect(coveredBy('drivers/gpu/drm', ticked)).toBe(true);
    expect(coveredBy('drivers/gpu', ticked)).toBe(false);
    expect(coveredBy('drivers/gpu2', ticked)).toBe(false);
  });
});

describe('a folder partly in the cone', () => {
  it('is one with a ticked folder somewhere below it, however deep', () => {
    const ticked = new Set(['drivers/gpu/drm']);
    expect(holds('drivers', ticked)).toBe(true);
    expect(holds('drivers/gpu', ticked)).toBe(true);
    expect(holds('drivers/gpu/drm', ticked)).toBe(false);
    // A shared prefix is not a parent.
    expect(holds('driver', ticked)).toBe(false);
  });
});
