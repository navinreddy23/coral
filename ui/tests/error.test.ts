import { describe, expect, it } from 'vitest';

import { codeOf, messageOf } from '../src/ipc/error';

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
