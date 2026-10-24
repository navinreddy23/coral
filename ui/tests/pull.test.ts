import { describe, expect, it } from 'vitest';

import { inTheWay } from '../src/app/pull';

describe('uncommitted work in the way of a pull', () => {
  it('names the files git names', () => {
    const said =
      'error: Your local changes to the following files would be overwritten by merge:\n' +
      '\tREADME.md\n\tsrc/lib.rs\n' +
      'Please commit your changes or stash them before you merge.\nAborting';
    expect(inTheWay(said)).toEqual(['README.md', 'src/lib.rs']);
  });

  it('reads the line the window keeps, which has had git\'s advice taken off', () => {
    const kept = 'Your local changes to the following files would be overwritten by merge:\n\tREADME.md';
    expect(inTheWay(kept)).toEqual(['README.md']);
  });

  it('is not fooled by any other failure', () => {
    expect(inTheWay('fatal: unable to access the remote')).toBeNull();
    expect(inTheWay(undefined)).toBeNull();
  });
});
