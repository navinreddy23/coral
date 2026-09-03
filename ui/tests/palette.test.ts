import { describe, expect, it } from 'vitest';

import { rankCommand } from '../src/app/palette';

describe('command ranking', () => {
  it('matches a subsequence, not just a prefix', () => {
    expect(rankCommand('checkout branch', 'cob')).not.toBeNull();
    expect(rankCommand('checkout branch', 'zz')).toBeNull();
  });

  it('prefers a tighter match', () => {
    // "push" packed together beats the same letters scattered through another label.
    const tight = rankCommand('push', 'push');
    const loose = rankCommand('pull, rebasing to stash', 'push');
    expect(tight).not.toBeNull();
    if (tight !== null && loose !== null) expect(tight).toBeLessThan(loose);
  });

  it('prefers an earlier match when two are equally tight', () => {
    const early = rankCommand('push now', 'push');
    const late = rankCommand('force push', 'push');
    expect(early).not.toBeNull();
    if (early !== null && late !== null) expect(early).toBeLessThan(late);
  });

  it('matches everything on an empty query', () => {
    expect(rankCommand('anything', '')).not.toBeNull();
  });

  it('needs every character of the query, in order', () => {
    expect(rankCommand('merge', 'gem')).toBeNull();
  });
});
