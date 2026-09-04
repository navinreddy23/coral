import { describe, expect, it } from 'vitest';

import { initialsOf } from '../src/graph/initials';

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
