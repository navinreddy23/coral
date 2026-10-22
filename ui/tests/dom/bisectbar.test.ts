// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));

import BisectBar from '../../src/app/BisectBar.svelte';
import { BisectState } from '../../src/state/bisect.svelte';
import type { Bisection } from '../../src/ipc/types';

afterEach(cleanup);

const BAD = 'b'.repeat(40);
const GOOD = 'a'.repeat(40);
const TESTING = 'c'.repeat(40);

function bisection(over: Partial<Bisection> = {}): Bisection {
  return {
    terms: { bad: 'bad', good: 'good' },
    bad: BAD,
    good: [GOOD],
    skipped: [],
    testing: TESTING,
    suspects: 7,
    steps: 3,
    found: null,
    onlySkippedLeft: false,
    mistaken: false,
    verdicts: [],
    ...over,
  };
}

function bar(state: Bisection | null) {
  const bisect = new BisectState();
  bisect.current = state;
  const calls = { mark: [] as string[], end: 0, shown: [] as string[] };
  const view = render(BisectBar, {
    props: {
      bisect,
      busy: false,
      subjectOf: (oid: string) => (oid === BAD ? 'step 5: break it' : null),
      onMark: (v: string) => calls.mark.push(v),
      onEnd: () => (calls.end += 1),
      onShow: (oid: string) => calls.shown.push(oid),
    },
  });
  const button = (name: string) =>
    [...view.container.querySelectorAll('button')].find((b) => b.textContent?.trim() === name);
  return { ...view, calls, button };
}

describe('the bisect bar', () => {
  it('is not there when nothing is being bisected', () => {
    expect(bar(null).container.querySelector('.bisect')).toBeNull();
  });

  it('says what is being tested and takes a verdict on it', async () => {
    const { container, calls, button } = bar(bisection());
    expect(container.textContent).toContain('Testing');
    expect(container.textContent).toContain('cccccccc');
    expect(container.textContent).toContain('about 3 more steps after this, 7 suspects');
    await fireEvent.click(button('Good') as HTMLElement);
    await fireEvent.click(button('Bad') as HTMLElement);
    await fireEvent.click(button('Skip') as HTMLElement);
    await fireEvent.click(button('End bisect') as HTMLElement);
    expect(calls.mark).toEqual(['good', 'bad', 'skip']);
    expect(calls.end).toBe(1);
  });

  it('says the commit being tested is the last when git counts no steps after it', () => {
    const { container } = bar(bisection({ steps: 0, suspects: 2 }));
    expect(container.textContent).toContain('the last to test, 2 suspects');
    expect(container.textContent).not.toContain('0 more');
  });

  it('asks for the side it is missing, and offers no verdict until then', () => {
    const missingGood = bar(bisection({ good: [], testing: null, suspects: null, steps: null }));
    expect(missingGood.container.textContent).toContain('Now mark one that was good');
    expect(missingGood.button('Good')).toBeUndefined();
    cleanup();
    const missingBad = bar(bisection({ bad: null, testing: null, suspects: null, steps: null }));
    expect(missingBad.container.textContent).toContain('Mark a commit that is bad');
  });

  it('names the culprit and goes to it', async () => {
    const { container, calls, button } = bar(bisection({ found: BAD, testing: null, suspects: 1 }));
    expect(container.textContent).toContain('First bad commit');
    expect(container.textContent).toContain('step 5: break it');
    expect(button('Good')).toBeUndefined();
    await fireEvent.click(button('bbbbbbbb') as HTMLElement);
    expect(calls.shown).toEqual([BAD]);
  });

  it('says when only skipped commits are left', () => {
    const { container } = bar(bisection({ onlySkippedLeft: true, testing: null }));
    expect(container.textContent).toContain('Only skipped commits are left');
  });

  it('says good and bad look swapped, and still offers the way out', async () => {
    const { container, calls, button } = bar(
      bisection({ mistaken: true, testing: null, suspects: null, steps: null }),
    );
    expect(container.textContent?.replace(/\s+/g, ' ')).toContain('as if the two were swapped');
    expect(button('Good')).toBeUndefined();
    await fireEvent.click(button('End bisect') as HTMLElement);
    expect(calls.end).toBe(1);
  });

  it('speaks in the terms the bisect was started with', () => {
    const { button } = bar(bisection({ terms: { bad: 'new', good: 'old' } }));
    expect(button('Old')).toBeDefined();
    expect(button('New')).toBeDefined();
  });
});

describe('the verdicts given so far', () => {
  it('include an earlier bad commit the refs no longer hold', () => {
    // git moves its one bad ref to the newest bad commit; the log still has the first.
    const earlier = 'e'.repeat(40);
    const bisect = new BisectState();
    bisect.current = bisection({
      verdicts: [
        { oid: earlier, verdict: 'bad' },
        { oid: GOOD, verdict: 'good' },
        { oid: BAD, verdict: 'bad' },
      ],
    });
    expect(bisect.marks.get(earlier)).toBe('bad');
    expect(bisect.marks.get(BAD)).toBe('bad');
  });

  it('are known by commit, the bad one winning a commit marked twice', () => {
    const bisect = new BisectState();
    bisect.current = bisection({ skipped: ['d'.repeat(40)] });
    expect(bisect.marks.get(BAD)).toBe('bad');
    expect(bisect.marks.get(GOOD)).toBe('good');
    expect(bisect.marks.get('d'.repeat(40))).toBe('skip');
    expect(bisect.marks.get(TESTING)).toBeUndefined();
  });
});

describe('reading the bisection', () => {
  it('keeps the answer for the repository asked about last, whichever arrives last', async () => {
    const { invoke } = await import('../../src/ipc/invoke');
    const mock = vi.mocked(invoke);
    let releaseSlow: (v: unknown) => void = () => undefined;
    mock.mockImplementation(async (_cmd, args) => {
      const path = (args as { path: string }).path;
      if (path === '/slow') return new Promise((r) => (releaseSlow = r));
      return null;
    });
    const bisect = new BisectState();
    const slow = bisect.load('/slow');
    await bisect.load('/fast');
    releaseSlow(bisection());
    await slow;
    expect(bisect.current).toBeNull();
  });
});
