// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';

import { face } from '../src/graph/face';

function picture(size: number): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = size;
  c.height = size;
  return c;
}

describe("an author's picture in a node", () => {
  it('is shrunk to the size the node draws it, once', () => {
    const source = picture(64);
    const small = face(source, 18) as HTMLCanvasElement;
    expect(small).not.toBe(source);
    expect([small.width, small.height]).toEqual([18, 18]);
    expect(face(source, 18), 'kept, not made again').toBe(small);
  });

  it('is drawn as it is when it is already small enough', () => {
    const source = picture(64);
    expect(face(source, 40)).toBe(source);
  });
});
