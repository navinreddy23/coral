import { describe, expect, it, vi } from 'vitest';

import { askUntilAccepted } from '../src/app/prompt';

describe('asking for a name git has the last word on', () => {
  it('asks again with the refused name still in the box', async () => {
    const asked: string[] = [];
    const ask = vi.fn(async (initial: string) => {
      asked.push(initial);
      return asked.length === 1 ? 'bad name' : 'good-name';
    });
    const run = vi.fn(async (text: string) => text === 'good-name');

    await askUntilAccepted('', ask, run);

    // Second time round the box holds what git refused, not an empty field.
    expect(asked).toEqual(['', 'bad name']);
    expect(run.mock.calls.map((c) => c[0])).toEqual(['bad name', 'good-name']);
  });

  it('stops the moment the work is accepted', async () => {
    const ask = vi.fn(async () => 'fine');
    const run = vi.fn(async () => true);
    await askUntilAccepted('', ask, run);
    expect(ask).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('stops when the dialog is cancelled, without running anything', async () => {
    const run = vi.fn(async () => true);
    await askUntilAccepted('', async () => null, run);
    expect(run).not.toHaveBeenCalled();
  });

  it('stops on an empty box rather than asking git about nothing', async () => {
    const run = vi.fn(async () => true);
    await askUntilAccepted('', async () => '   ', run);
    expect(run).not.toHaveBeenCalled();
  });

  it('opens with what it was given, which is how a rename shows the old name', async () => {
    const asked: string[] = [];
    await askUntilAccepted('old-name', async (i) => {
      asked.push(i);
      return 'new-name';
    }, async () => true);
    expect(asked).toEqual(['old-name']);
  });

  it('hands on the trimmed name, so a stray space is not what git refuses', async () => {
    const run = vi.fn(async () => true);
    await askUntilAccepted('', async () => '  spaced  ', run);
    expect(run).toHaveBeenCalledWith('spaced');
  });
});
