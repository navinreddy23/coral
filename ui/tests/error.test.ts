import { describe, expect, it } from 'vitest';

import { codeOf, messageOf, whatFailed } from '../src/ipc/error';

describe('reading what went wrong', () => {
  it('reads the engine’s own message off a rejected command', () => {
    // A Tauri command that fails rejects with the serialized `IpcError`: a plain object, not
    // an `Error`. `String` of one is `[object Object]`, and that is what every failure in the
    // window showed — a terminal that would not start, a push that was rejected, all of it.
    const rejected = { code: 'refused', message: 'cannot terminal: /bin/sh is not there' };
    expect(messageOf(rejected)).toBe('cannot terminal: /bin/sh is not there');
    expect(messageOf(rejected)).not.toContain('object Object');
  });

  it('falls back to the code when there is no message', () => {
    expect(messageOf({ code: 'git_missing' })).toBe('git_missing');
  });

  it('still says something for a shape it has never seen', () => {
    // Its own shape beats its type name: `[object Object]` tells nobody anything.
    const said = messageOf({ unexpected: 42 });
    expect(said).toContain('42');
    expect(said).not.toContain('object Object');
  });

  it('handles the ordinary cases', () => {
    expect(messageOf(new Error('boom'))).toBe('boom');
    expect(messageOf('just a string')).toBe('just a string');
    expect(messageOf(null)).toBe('null');
    expect(messageOf(undefined)).toBe('undefined');
  });

  it('survives something that cannot be stringified', () => {
    const circular: Record<string, unknown> = {};
    circular['self'] = circular;
    // JSON.stringify throws on this; the reader must not.
    expect(() => messageOf(circular)).not.toThrow();
    expect(messageOf(circular)).toBe('[object Object]');
  });

  it('prefers an empty-string message to nothing, but not to a code', () => {
    expect(messageOf({ code: 'refused', message: '' })).toBe('refused');
  });

  it('hands back the code for deciding, and null when there is none', () => {
    expect(codeOf({ code: 'not_a_repository', message: 'x' })).toBe('not_a_repository');
    expect(codeOf(new Error('boom'))).toBeNull();
    expect(codeOf('a string')).toBeNull();
  });
});

describe('taking git\'s plumbing out of what the reader is shown', () => {
  /**
   * The engine's message is right for the activity log, the CLI envelope and a bug report:
   * `git tag exited with 128: fatal: 'a release' is not a valid tag name.` names the command
   * and the exit code. A toast is none of those, and the reader has to step over eight words
   * of machinery to reach the sentence that tells them what to type instead.
   */
  it('drops the command and the exit code, and the word git prints before its reason', () => {
    expect(
      messageOf({
        code: 'git_error',
        message: "git tag exited with 128: fatal: 'a release' is not a valid tag name.",
      }),
    ).toBe("'a release' is not a valid tag name.");
  });

  it('drops error: as well, which is the other word git leads with', () => {
    expect(
      messageOf({ code: 'git_error', message: 'git push exited with 1: error: failed to push' }),
    ).toBe('failed to push');
  });

  it('leaves the lines after the first alone, which carry the detail', () => {
    const said = messageOf({
      code: 'git_error',
      message: 'git merge exited with 1: fatal: refusing to merge\nerror: and here is why',
    });
    expect(said).toBe('refusing to merge\nerror: and here is why');
  });

  it('keeps a message that is only the machinery, rather than showing nothing at all', () => {
    expect(messageOf({ code: 'git_error', message: 'git gc exited with 1: ' })).toBe(
      'git gc exited with 1:',
    );
  });

  it('drops the word on its own too, since only git writes it', () => {
    // Nothing the engine says begins `fatal:` or `error:`, so there is no message of Coral's
    // own for this to eat into.
    expect(messageOf({ code: 'git_error', message: 'fatal: not a git repository' })).toBe(
      'not a git repository',
    );
  });

  it('leaves every other message exactly as the engine wrote it', () => {
    expect(messageOf({ code: 'refused', message: 'the repository is not there' })).toBe(
      'the repository is not there',
    );
    expect(messageOf({ code: 'refused', message: 'error while it is not a prefix' })).toBe(
      'error while it is not a prefix',
    );
  });
});

describe('naming the operation a failure belongs to', () => {
  /**
   * A red toast titled "Something went wrong" says nothing the colour has not already said.
   * The engine names every action for the journal — `tag v1.0`, `push main` — and that name is
   * what the toast should be titled with, matching the wording a success gets.
   */
  it('reads the label the engine attached to the failure', () => {
    expect(whatFailed({ code: 'git_error', message: 'no', what: 'tag v1.0' })).toBe('tag v1.0');
  });

  it('answers null when there is none, so the caller can say something general', () => {
    expect(whatFailed({ code: 'git_error', message: 'no' })).toBeNull();
    expect(whatFailed({ code: 'git_error', message: 'no', what: '' })).toBeNull();
    expect(whatFailed('a plain string')).toBeNull();
    expect(whatFailed(null)).toBeNull();
  });
});
