// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  isPreview: () => false,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { hitSpans } from '../src/app/hit';
import { GrepState } from '../src/state/grep.svelte';

const AT = { repo: '/repo', rev: 'abc123', label: 'abc123' };

function hit(path: string, line: number, text: string) {
  return { path, line, column: 1, text };
}

beforeEach(() => {
  invoke.mockReset();
  vi.useRealTimers();
});

describe('marking what a search found', () => {
  it('marks text as text, every time it appears', () => {
    expect(hitSpans('f(a) + f(a)', 'f(a)', 'fixed', false)).toEqual([
      { text: 'f(a)', marked: true },
      { text: ' + ', marked: false },
      { text: 'f(a)', marked: true },
    ]);
  });

  it('matches across case when asked to, and an expression as one', () => {
    expect(hitSpans('Count count', 'COUNT', 'fixed', true)?.filter((s) => s.marked)).toHaveLength(2);
    expect(hitSpans('Count count', 'COUNT', 'fixed', false)).toBeNull();
    expect(hitSpans('a1 b22', '[0-9]+', 'regex', false)).toEqual([
      { text: 'a', marked: false },
      { text: '1', marked: true },
      { text: ' b', marked: false },
      { text: '22', marked: true },
    ]);
  });

  it('leaves a line unmarked rather than marking it wrong', () => {
    // POSIX classes are git's, not JavaScript's.
    expect(hitSpans('abc', '(', 'regex', false)).toBeNull();
    expect(hitSpans('abc', 'x*', 'regex', false)).toBeNull();
  });
});

describe('the code search', () => {
  it('searches a moment after the typing stops, once, for what was typed last', async () => {
    vi.useFakeTimers();
    invoke.mockResolvedValue({ matches: [hit('a.c', 3, 'int count;')], more: false });
    const grep = new GrepState();
    grep.open(AT);
    grep.set({ pattern: 'c' });
    grep.set({ pattern: 'co' });
    grep.set({ pattern: 'count', paths: ' src  *.c ' });
    expect(invoke).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(300);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0]).toEqual([
      'grep_code',
      {
        path: '/repo',
        rev: 'abc123',
        query: { pattern: 'count', how: 'fixed', ignoreCase: true, paths: ['src', '*.c'] },
      },
    ]);
    expect(grep.result?.matches).toHaveLength(1);
    expect(grep.searching).toBe(false);
  });

  it('lets a stopped or overtaken search go without a word', async () => {
    let answer: (v: unknown) => void = () => {};
    let refuse: (e: unknown) => void = () => {};
    invoke.mockImplementationOnce(
      () => new Promise((resolve, reject) => {
        answer = resolve;
        refuse = reject;
      }),
    );
    invoke.mockResolvedValueOnce({ matches: [hit('b.c', 1, 'two')], more: false });
    const grep = new GrepState();
    grep.open(AT);
    grep.pattern = 'one';
    const first = grep.run();
    grep.pattern = 'two';
    await grep.run();
    refuse({ code: 'cancelled', message: 'cancelled' });
    answer({ matches: [hit('a.c', 1, 'one')], more: false });
    await first;
    expect(grep.result?.matches[0]?.text).toBe('two');
    expect(grep.error).toBeNull();
    expect(grep.asked?.pattern).toBe('two');
  });

  it('says what git refused, and groups what it found by file', async () => {
    invoke.mockRejectedValueOnce({ code: 'git_error', message: 'fatal: Unmatched ( or \\(' });
    const grep = new GrepState();
    grep.open(AT);
    grep.pattern = '(';
    await grep.run();
    expect(grep.error).toContain('Unmatched');

    invoke.mockResolvedValueOnce({
      matches: [hit('a.c', 1, 'x'), hit('a.c', 9, 'x'), hit('b.c', 2, 'x')],
      more: true,
    });
    grep.pattern = 'x';
    await grep.run();
    expect(grep.error).toBeNull();
    expect(grep.files.map((f) => [f.path, f.matches.length])).toEqual([['a.c', 2], ['b.c', 1]]);
  });

  it('keeps what to find in another repository, but not the paths to look under', () => {
    invoke.mockResolvedValue(false);
    const grep = new GrepState();
    grep.open(AT);
    grep.pattern = 'needle';
    grep.paths = 'kernel';
    grep.close();
    grep.open({ ...AT, rev: 'def456' });
    expect(grep.paths).toBe('kernel');
    grep.close();
    grep.open({ repo: '/other', rev: 'HEAD', label: 'main' });
    expect(grep.pattern).toBe('needle');
    expect(grep.paths).toBe('');
  });

  it('keeps the answer for the same revision and starts again for another', async () => {
    invoke.mockResolvedValue({ matches: [hit('a.c', 1, 'x')], more: false });
    const grep = new GrepState();
    grep.open(AT);
    grep.pattern = 'x';
    await grep.run();
    grep.open({ ...AT, label: 'the same commit' });
    expect(grep.result).not.toBeNull();
    vi.useFakeTimers();
    grep.open({ ...AT, rev: 'def456' });
    expect(grep.result).toBeNull();
    await vi.advanceTimersByTimeAsync(300);
    expect(invoke.mock.calls.at(-1)?.[1]).toMatchObject({ rev: 'def456', query: { pattern: 'x' } });
  });
});
