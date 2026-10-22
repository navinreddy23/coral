import { repoBisection } from '../ipc/bisect';
import type { Bisection, Verdict } from '../ipc/types';

/**
 * The bisection in the repository, if one is running.
 *
 * Read from git each time rather than kept here, so one started or moved on in the terminal
 * beside the window is the one the window shows.
 */
export class BisectState {
  current = $state<Bisection | null>(null);

  active = $derived(this.current !== null);

  /**
   * Every commit given a verdict, by object id, so a row can say which.
   *
   * From the log, which has all of them, with the refs over it: git keeps one bad ref and
   * moves it, so an earlier bad commit is only in the log, and the refs win where the two
   * differ because they are what git will act on.
   */
  marks = $derived.by(() => {
    const out = new Map<string, Verdict>();
    const b = this.current;
    if (b === null) return out;
    for (const j of b.verdicts) out.set(j.oid, j.verdict);
    for (const oid of b.good) out.set(oid, 'good');
    for (const oid of b.skipped) out.set(oid, 'skip');
    if (b.bad !== null) out.set(b.bad, 'bad');
    return out;
  });

  /** Which read the held answer is for, so a slow one for another tab cannot replace it. */
  #token = 0;

  async load(path: string): Promise<void> {
    const token = ++this.#token;
    let found: Bisection | null;
    try {
      found = await repoBisection(path);
    } catch {
      // Unreadable is treated as none: the bar offers nothing that would act on a guess, and
      // the next change in the repository reads it again.
      found = null;
    }
    // A bisect read for the tab left behind, landing after the one switched to, would put that
    // repository's verdict buttons over this one, and they act on whichever is open.
    if (token === this.#token) this.current = found;
  }

  /** What to call a verdict, in the words this bisection was started with. */
  word(verdict: Verdict): string {
    const terms = this.current?.terms ?? { bad: 'bad', good: 'good' };
    if (verdict === 'skip') return 'skip';
    return verdict === 'bad' ? terms.bad : terms.good;
  }
}
