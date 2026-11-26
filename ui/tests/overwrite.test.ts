import { describe, expect, it } from 'vitest';

import { inTheWay, refusedToRebase } from '../src/app/overwrite';

describe('uncommitted work in the way of a pull or a checkout', () => {
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

  it('names them for a checkout too', () => {
    const said =
      'error: Your local changes to the following files would be overwritten by checkout:\n' +
      '\tCHANGES.md\nPlease commit your changes or stash them before you switch branches.\nAborting';
    expect(inTheWay(said)).toEqual(['CHANGES.md']);
  });

  it('is not fooled by any other failure', () => {
    expect(inTheWay('fatal: unable to access the remote')).toBeNull();
    expect(inTheWay(undefined)).toBeNull();
  });
});

// git will not rebase over uncommitted work at all, whatever the pull changes, and says so in
// words of its own that name no file. The pull ended in that message.
describe('a pull that will not rebase over uncommitted work', () => {
  it('is told by what git says about the working copy', () => {
    expect(
      refusedToRebase(
        'error: cannot pull with rebase: You have unstaged changes.\nerror: Please commit or stash them.',
      ),
    ).toBe(true);
    expect(
      refusedToRebase(
        'error: cannot pull with rebase: Your index contains uncommitted changes.\n' +
          'error: Please commit or stash them.',
      ),
    ).toBe(true);
  });

  it('is not fooled by any other failure', () => {
    expect(refusedToRebase('error: could not apply 6b4b991... docs: say x')).toBe(false);
    expect(refusedToRebase(undefined)).toBe(false);
  });
});
