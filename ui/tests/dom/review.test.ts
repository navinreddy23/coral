// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('../../src/ipc/commands', () => ({ hostingReview: vi.fn(), hostingReply: vi.fn() }));

import Review from '../../src/app/Review.svelte';
import { ReviewState } from '../../src/state/review.svelte';
import type { PullRequest } from '../../src/ipc/commands';

const pr: PullRequest = {
  number: 7,
  title: 'Draw lanes faster',
  state: 'open',
  author: 'ada',
  sourceBranch: 'feature/lanes',
  targetBranch: 'main',
  webUrl: 'https://example.test/7',
  updatedAt: '2026-10-02T10:00:00Z',
  headSha: '',
};

function shown(canRequestChanges: boolean) {
  const review = new ReviewState();
  review.remarks = [
    { author: 'bob', body: 'Off by one?', at: '2026-10-02T09:00:00Z', verdict: null, path: 'src/lanes.rs', line: 42 },
    { author: 'cy', body: '', at: '2026-10-02T10:00:00Z', verdict: 'approved', path: null, line: null },
  ];
  return render(Review, {
    props: { review, pr, label: 'Pull request', canRequestChanges, onClose: vi.fn() },
  });
}

describe('the review panel', () => {
  it('shows who said what, where, and what they decided', () => {
    const { container } = shown(true);
    const items = [...container.querySelectorAll('.remarks li')];
    expect(items[0]?.textContent).toContain('src/lanes.rs:42');
    expect(items[0]?.textContent).toContain('Off by one?');
    expect(items[1]?.classList.contains('approved')).toBe(true);
    expect(items[1]?.textContent).toContain('approved');
  });

  it('asks for words before commenting or requesting changes, but not before approving', () => {
    const { container } = shown(true);
    const button = (label: string) =>
      [...container.querySelectorAll('.acts button')].find((b) => b.textContent?.trim() === label) as
        | HTMLButtonElement
        | undefined;
    expect(button('Comment')?.disabled).toBe(true);
    expect(button('Request changes')?.disabled).toBe(true);
    expect(button('Approve')?.disabled).toBe(false);
  });

  it('offers no request for changes where the host has none', () => {
    const { container } = shown(false);
    expect(container.textContent).not.toContain('Request changes');
  });
});
