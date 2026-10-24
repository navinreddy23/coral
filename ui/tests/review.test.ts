import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls = vi.hoisted(() => ({ review: vi.fn(), reply: vi.fn() }));
vi.mock('../src/ipc/commands', () => ({
  hostingReview: calls.review,
  hostingReply: calls.reply,
}));

import { ReviewState } from '../src/state/review.svelte';

const said = {
  author: 'ada',
  body: 'Needs a test',
  at: '2026-10-02T10:00:00Z',
  verdict: 'changesRequested' as const,
  path: null,
  line: null,
};

describe('a proposal\'s review', () => {
  beforeEach(() => {
    calls.review.mockReset();
    calls.reply.mockReset();
  });

  it('reads what was said, and reads it again after a reply', async () => {
    calls.review.mockResolvedValueOnce([said]).mockResolvedValueOnce([said, { ...said, verdict: null }]);
    calls.reply.mockResolvedValue(undefined);
    const review = new ReviewState();
    await review.load('/repo', 7);
    expect(review.remarks).toHaveLength(1);
    expect(await review.reply({ kind: 'comment', body: 'Added one' })).toBe(true);
    expect(calls.reply).toHaveBeenCalledWith('/repo', 7, { kind: 'comment', body: 'Added one' });
    expect(review.remarks).toHaveLength(2);
  });

  it('keeps what was said when a reply is refused, and says why', async () => {
    calls.review.mockResolvedValue([said]);
    calls.reply.mockRejectedValue(new Error('GitLab has no way to request changes'));
    const review = new ReviewState();
    await review.load('/repo', 7);
    expect(await review.reply({ kind: 'requestChanges', body: 'x' })).toBe(false);
    expect(review.error).toContain('no way to request changes');
    expect(review.remarks).toEqual([said]);
  });
});
