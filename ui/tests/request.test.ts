import { describe, expect, it } from 'vitest';
import { quoteRequest } from '../src/app/request';

describe('quoting a request number', () => {
  it('writes a merge request the way GitLab does, and a pull request the way GitHub does', () => {
    expect(quoteRequest('gitlab', 42)).toBe('!42');
    expect(quoteRequest('github', 42)).toBe('#42');
    expect(quoteRequest(undefined, 7)).toBe('#7');
  });
});
