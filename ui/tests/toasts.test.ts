import { describe, expect, it, vi } from 'vitest';

import { describe as phrase, ToastsState } from '../src/state/toasts.svelte';

describe('toasts', () => {
  it('stacks rather than replacing, because a fetch of three remotes has three answers', () => {
    const toasts = new ToastsState();
    toasts.push('ok', 'origin fetched');
    toasts.push('error', 'upstream refused');

    expect(toasts.items.map((t) => t.title)).toEqual(['origin fetched', 'upstream refused']);
  });

  it('closes one without touching the rest', () => {
    const toasts = new ToastsState();
    const first = toasts.push('ok', 'one');
    toasts.push('ok', 'two');

    toasts.dismiss(first);
    expect(toasts.items.map((t) => t.title)).toEqual(['two']);
  });

  it('keeps only the newest few, so a burst cannot fill the window', () => {
    const toasts = new ToastsState();
    for (let i = 0; i < 9; i++) toasts.push('info', `toast ${i}`);

    expect(toasts.items).toHaveLength(ToastsState.MAX);
    expect(toasts.items[0]?.title).toBe('toast 4');
    expect(toasts.items.at(-1)?.title).toBe('toast 8');
  });

  it('dismisses a success by itself but leaves a failure up', () => {
    vi.useFakeTimers();
    try {
      const toasts = new ToastsState();
      toasts.push('ok', 'pushed');
      toasts.push('error', 'rejected');

      vi.advanceTimersByTime(10_000);
      // A failure has to survive looking away from the window.
      expect(toasts.items.map((t) => t.title)).toEqual(['rejected']);
    } finally {
      vi.useRealTimers();
    }
  });

  it('clears every timer when the stack is emptied', () => {
    vi.useFakeTimers();
    try {
      const toasts = new ToastsState();
      toasts.push('ok', 'one');
      toasts.clear();
      toasts.push('error', 'two');

      // If the first toast's timer had survived, it would fire and clear this one too.
      vi.advanceTimersByTime(10_000);
      expect(toasts.items.map((t) => t.title)).toEqual(['two']);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('phrasing an outcome', () => {
  it('tells "already up to date" apart from a pull that brought commits', () => {
    // Both are successes, and reporting them the same way leaves the user unsure whether
    // anything happened at all.
    const quiet = phrase('pull', 'Already up to date.', false);
    expect(quiet.kind).toBe('info');
    expect(quiet.title).toContain('already up to date');

    const busy = phrase('pull', 'Updating a1b2c3..d4e5f6\nFast-forward', false);
    expect(busy.kind).toBe('ok');
    expect(busy.detail).toContain('Fast-forward');
  });

  it('marks a stop on conflicts as needing attention rather than as a success', () => {
    const said = phrase('merge', 'CONFLICT (content): Merge conflict in a.txt', true);
    expect(said.kind).toBe('warn');
    expect(said.title).toContain('stopped on conflicts');
  });

  it('accepts the hyphenated spelling git also uses', () => {
    expect(phrase('push', 'Everything up-to-date', false).kind).toBe('info');
  });
});

describe('what to call an outcome that did not complete', () => {
  it('calls a rejected push rejected, because nothing conflicted', () => {
    // The branch moved on the remote while this one was being written. The answer is to
    // fetch, not to resolve anything.
    const said = phrase(
      'push',
      'refs/heads/topic -> refs/heads/topic [rejected] (fetch first)',
      true,
    );
    expect(said.title).toBe('push was rejected');
    expect(said.kind).toBe('warn');
  });

  it('still calls a stopped rebase a conflict', () => {
    const said = phrase('rebase onto main', 'error: could not apply 1a2b3c4', true);
    expect(said.title).toBe('rebase onto main stopped on conflicts');
  });
});
