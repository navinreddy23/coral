import { describe, expect, it } from 'vitest';
import { stepZoom, ZOOMS } from '../src/app/zoom';

describe('zoom steps', () => {
  it('moves one step at a time and stops at either end', () => {
    expect(stepZoom(1, 1)).toBe(1.1);
    expect(stepZoom(1.1, 1)).toBe(1.25);
    expect(stepZoom(1, -1)).toBe(0.9);
    expect(stepZoom(ZOOMS[0], -1)).toBe(ZOOMS[0]);
    expect(stepZoom(2, 1)).toBe(2);
    // A scale that is not a step starts again from actual size.
    expect(stepZoom(1.37, 1)).toBe(1.1);
  });
});
