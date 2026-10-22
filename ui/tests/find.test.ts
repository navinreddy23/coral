import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('../src/ipc/invoke', () => ({ invoke: (...a: unknown[]) => invoke(...a), isPreview: () => false }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { FindState } from '../src/state/find.svelte';

beforeEach(() => {
  invoke.mockReset();
});

function found(rows: number[]) {
  return rows.map((row) => ({ oid: `${row}`.padStart(40, '0'), row }));
}

describe('finding a commit', () => {
  it('steps forward and back through the matches, wrapping at each end', async () => {
    invoke.mockResolvedValue(found([4, 19, 40]));
    const find = new FindState();
    await find.run('/repo', 'parser');

    expect(find.matches).toHaveLength(3);
    expect(find.current).toBe(4);
    expect(find.step(1)).toBe(19);
    expect(find.step(1)).toBe(40);
    // Round the end rather than stopping on it: the count says how many there are, and
    // stopping dead on the last one reads as the button having broken.
    expect(find.step(1)).toBe(4);
    expect(find.step(-1)).toBe(40);
  });

  it('says nothing is current when nothing matched', async () => {
    invoke.mockResolvedValue([]);
    const find = new FindState();
    await find.run('/repo', 'nothing at all');
    expect(find.current).toBeNull();
    expect(find.step(1)).toBeNull();
  });

  it('drops an answer to a query nobody is asking any more', async () => {
    // A slow first search must not replace the results of the one typed after it.
    let release: (value: unknown) => void = () => {};
    invoke.mockImplementationOnce(() => new Promise((r) => (release = r)));
    invoke.mockResolvedValueOnce(found([7]));

    const find = new FindState();
    const slow = find.run('/repo', 'a');
    await find.run('/repo', 'ab');
    release(found([1, 2, 3]));
    await slow;

    expect(find.matches.map((m) => m.row)).toEqual([7]);
  });

  it('asks nothing of an empty query, and forgets what it held', async () => {
    invoke.mockResolvedValue(found([5]));
    const find = new FindState();
    await find.run('/repo', 'x');
    expect(find.matches).toHaveLength(1);

    find.type('/repo', '   ');
    expect(find.matches).toHaveLength(0);
    expect(find.searching).toBe(false);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('marks every matched row, so the list can tint them', async () => {
    invoke.mockResolvedValue(found([2, 9]));
    const find = new FindState();
    await find.run('/repo', 'x');
    expect([...find.rows].sort((a, b) => a - b)).toEqual([2, 9]);
  });

  it('closing forgets the search rather than leaving it tinting the list', async () => {
    invoke.mockResolvedValue(found([3]));
    const find = new FindState();
    find.show();
    await find.run('/repo', 'x');
    find.close();

    expect(find.open).toBe(false);
    expect(find.query).toBe('');
    expect(find.matches).toHaveLength(0);
  });
  it('asks for the newest search at once, and lets the one it replaced go quietly', async () => {
    // The engine stops the older search when the newer one arrives; it answers `cancelled`,
    // which is not something the reader did wrong and must not be shown as an error.
    let refuse: (reason: unknown) => void = () => {};
    invoke.mockImplementationOnce(() => new Promise((_, r) => (refuse = r)));
    invoke.mockResolvedValue(found([9]));

    const find = new FindState();
    const first = find.run('/repo', 'b');
    await find.run('/repo', 'bcm2835');
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke.mock.calls[1]?.[1]).toMatchObject({ query: 'bcm2835' });

    refuse({ code: 'cancelled', message: 'search was cancelled' });
    await first;
    expect(find.matches.map((m) => m.row)).toEqual([9]);
    expect(find.error).toBeNull();
    expect(find.searching).toBe(false);
  });

  it('stops the search still running when the box is closed or emptied', async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === 'stop_search' ? Promise.resolve(true) : new Promise(() => undefined),
    );
    const find = new FindState();
    find.show();
    void find.run('/kernel', 'code:render_row');
    find.close();
    expect(invoke).toHaveBeenCalledWith('stop_search', { path: '/kernel' });
    expect(find.searching).toBe(false);

    invoke.mockClear();
    void find.run('/kernel', 'code:x');
    find.type('/kernel', '');
    expect(invoke).toHaveBeenCalledWith('stop_search', { path: '/kernel' });
  });

  it('shows a qualifier it could not read', async () => {
    invoke.mockRejectedValue({ code: 'refused', message: 'cannot after: soon is not a date' });
    const find = new FindState();
    await find.run('/repo', 'after:soon');
    expect(find.error).toContain('not a date');
  });
});
