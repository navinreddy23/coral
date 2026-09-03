import { describe, expect, it } from 'vitest';

import { authorColourIndex, initialsOf } from '../src/graph/initials';

describe('initialsOf', () => {
  it('takes the first and last word', () => {
    expect(initialsOf('Linus Torvalds')).toBe('LT');
    expect(initialsOf('Greg Kroah-Hartman')).toBe('GH');
    expect(initialsOf('Ada')).toBe('A');
  });

  it('ignores middle names, so the surname always shows', () => {
    expect(initialsOf('Jean Baptiste Emanuel Zorg')).toBe('JZ');
  });

  it('reads a handle as a name', () => {
    expect(initialsOf('nr.reddy')).toBe('NR');
    expect(initialsOf('some_user_name')).toBe('SN');
  });

  it('has nothing to draw for an empty or symbol-only name', () => {
    expect(initialsOf('')).toBeNull();
    expect(initialsOf('   ')).toBeNull();
    expect(initialsOf('!!!')).toBeNull();
  });

  it('keeps a whole code point for names outside the BMP', () => {
    // A surrogate half would render as a replacement character on the node.
    expect(initialsOf('𝒜lice')).toBe('𝒜');
    expect([...(initialsOf('𝒜lice Bob') ?? '')]).toHaveLength(2);
  });

  it('leaves caseless scripts alone', () => {
    expect(initialsOf('田中 太郎')).toBe('田太');
  });
});

describe('authorColourIndex', () => {
  it('gives the same person the same colour every time', () => {
    const a = authorColourIndex('torvalds@linux-foundation.org', 8);
    const b = authorColourIndex('torvalds@linux-foundation.org', 8);
    expect(a).toBe(b);
  });

  it('stays inside the palette', () => {
    for (const who of ['a@b.c', 'someone-else@example.com', '', 'x'.repeat(200)]) {
      const at = authorColourIndex(who, 8);
      expect(at, who).toBeGreaterThanOrEqual(0);
      expect(at, who).toBeLessThan(8);
    }
  });

  it('spreads a realistic set of authors across the palette', () => {
    // All one colour would be worse than no colour: the point is telling runs apart.
    const authors = Array.from({ length: 40 }, (_, i) => `dev${i}@kernel.org`);
    const used = new Set(authors.map((a) => authorColourIndex(a, 8)));
    expect(used.size).toBeGreaterThanOrEqual(6);
  });

  it('does not divide by an empty palette', () => {
    expect(authorColourIndex('a@b.c', 0)).toBe(0);
  });
});
