import { afterEach, describe, expect, it, vi } from 'vitest';

import { shortAge } from '../src/app/age';

/** Seconds, `ago` ago, against a clock this test pins. */
const NOW = 1_800_000_000;
const ago = (seconds: number) => NOW - seconds;

function at(now: number) {
  vi.useFakeTimers();
  vi.setSystemTime(now * 1000);
}

afterEach(() => vi.useRealTimers());

describe('how long ago something happened', () => {
  it('says so in minutes for the first hour', () => {
    // The commit somebody just recorded is the top row of the list they are looking at, and
    // it used to read "1h": everything under an hour was rounded up to one, so a commit four
    // minutes old claimed to be an hour old on the screen that had just made it.
    at(NOW);
    expect(shortAge(ago(4 * 60))).toBe('4m');
    expect(shortAge(ago(45 * 60))).toBe('45m');
  });

  it('has a word for something that has only just happened', () => {
    at(NOW);
    expect(shortAge(ago(3))).toBe('now');
    expect(shortAge(ago(0))).toBe('now');
  });

  it('moves to hours at the hour and to days at the day', () => {
    at(NOW);
    expect(shortAge(ago(60 * 60))).toBe('1h');
    expect(shortAge(ago(5 * 60 * 60))).toBe('5h');
    expect(shortAge(ago(30 * 60 * 60))).toBe('1d');
    expect(shortAge(ago(9 * 24 * 60 * 60))).toBe('9d');
  });

  it('moves to years past one, to a decimal, which is what a kernel tag needs', () => {
    at(NOW);
    expect(shortAge(ago(400 * 24 * 60 * 60))).toBe('1.1y');
    expect(shortAge(ago(21 * 365 * 24 * 60 * 60))).toBe('21.0y');
  });

  it('does not go backwards for a commit whose clock is ahead of this one', () => {
    // Commit times come from whatever machine made them, so a repository pulled from a host
    // with a fast clock has commits in the future. "now" is wrong by less than the skew;
    // a negative number of minutes is wrong by a century.
    at(NOW);
    expect(shortAge(NOW + 90)).toBe('now');
  });
});
