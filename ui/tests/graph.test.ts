// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { GraphState } from '../src/state/graph.svelte';

const frameBytes = readFileSync(resolve(process.cwd(), 'tests/fixtures/frame.bin'));

/** A fresh copy each time: the decoder takes ownership of the buffer it is handed. */
function frame(): ArrayBuffer {
  return frameBytes.buffer.slice(
    frameBytes.byteOffset,
    frameBytes.byteOffset + frameBytes.byteLength,
  );
}

/**
 * How the graph reloads.
 *
 * The row list is absolutely positioned inside the scroller against the scroll offset the
 * window holds, so the frame going to null unmounts that element and what comes back has an
 * offset of zero while the list is still placed for the old one. Fast-forwarding a tag while
 * reading row 2,900 of 3,000 therefore drew every row eighty thousand pixels below the
 * viewport and left an empty pane. What follows is where the frame may go and where it may not.
 */
function wire(watch?: (firstPaint: boolean) => void) {
  invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === 'binary_self_test') {
      return Uint8Array.from({ length: 4096 }, (_, i) => i % 251).buffer;
    }
    if (cmd === 'graph_rewalk') return null;
    if (cmd === 'graph_frame') {
      const first = (args as { firstPaint?: boolean }).firstPaint === true;
      watch?.(first);
      return frame();
    }
    if (cmd === 'row_metadata') return [];
    throw new Error(`unstubbed ${cmd}`);
  });
}

describe('reloading the graph', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('blanks the frame when the repository itself changes', async () => {
    // The other repository's commits under this one's name is worse than a loading screen, and
    // the loading screen is what asks whether there is a frame at all.
    const graph = new GraphState();
    wire();
    await graph.open('/alpha');
    expect(graph.frame).not.toBeNull();

    const seen: (unknown | null)[] = [];
    wire(() => seen.push(graph.frame));
    await graph.open('/beta');
    expect(seen[0], 'nothing of the old repository is left up').toBeNull();
  });

  it('keeps the rows up when the same repository is walked again', async () => {
    const graph = new GraphState();
    wire();
    await graph.open('/alpha');
    const before = graph.frame;

    const seen: (unknown | null)[] = [];
    wire(() => seen.push(graph.frame));
    await graph.open('/alpha');
    expect(seen.every((f) => f !== null), 'never empty in between').toBe(true);
    expect(before).not.toBeNull();
  });

  it('keeps the rows up even when the walk is about to change shape', async () => {
    // The reported bug. `forget` says the rows are about to mean a different set of commits,
    // which earns the fast first paint — and used to blank the frame to get it.
    const graph = new GraphState();
    wire();
    await graph.open('/alpha');

    const seen: (unknown | null)[] = [];
    wire(() => seen.push(graph.frame));
    graph.forget();
    await graph.open('/alpha');
    expect(seen.every((f) => f !== null), 'never empty in between').toBe(true);
  });

  it('takes the fast first paint after something moved a ref', async () => {
    // Without it the exact walk is the only thing that repaints, so a pull that brought four
    // months of the kernel left the old tip on screen for the fifty seconds it took.
    const graph = new GraphState();
    wire();
    await graph.open('/alpha');

    const asked: boolean[] = [];
    wire((first) => asked.push(first));
    graph.forget();
    await graph.open('/alpha');
    expect(asked, 'the fast frame, then the exact one').toEqual([true, false]);
  });

  it('skips the fast first paint on a plain reload of the same walk', async () => {
    // It would only replace the rows with commit-time order and then replace that again.
    const graph = new GraphState();
    wire();
    await graph.open('/alpha');

    const asked: boolean[] = [];
    wire((first) => asked.push(first));
    await graph.open('/alpha');
    expect(asked).toEqual([false]);
  });

  it('spends the fast first paint once, not on every reload after it', async () => {
    const graph = new GraphState();
    wire();
    await graph.open('/alpha');
    graph.forget();
    await graph.open('/alpha');

    const asked: boolean[] = [];
    wire((first) => asked.push(first));
    await graph.open('/alpha');
    expect(asked).toEqual([false]);
  });
});

describe('asking for rows that are not loaded yet', () => {
  const TOTAL = 200_000;
  const WANT = 150_000;

  beforeEach(() => {
    invoke.mockReset();
  });

  /**
   * The fixture's header says five rows starting at zero. This claims a repository worth
   * paging, and a window that begins where the caller is looking — which is what the engine
   * answers with, and what `covers` is asked about.
   */
  function holding(startRow: number): ArrayBuffer {
    const buf = frame();
    const view = new DataView(buf);
    view.setUint32(8, startRow, true);
    view.setUint32(16, TOTAL, true);
    return buf;
  }

  function serve(answer: (start: number, nth: number) => ArrayBuffer, count: () => void) {
    let nth = 0;
    invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === 'binary_self_test') {
        return Uint8Array.from({ length: 4096 }, (_, i) => i % 251).buffer;
      }
      if (cmd === 'graph_rewalk') return null;
      if (cmd === 'row_metadata') return [];
      if (cmd === 'graph_frame') {
        nth += 1;
        count();
        return answer(Number((args as { startRow?: number }).startRow ?? 0), nth);
      }
      throw new Error(`unstubbed ${cmd}`);
    });
  }

  /**
   * The window scrolls to a row and then asks for it before selecting.
   *
   * Scrolling starts the fetch for that window itself, so by the time the caller asked, a
   * request for the same start was already in flight — and the answer was to return at once,
   * as though the rows had arrived.
   */
  it('waits for a frame already on its way rather than answering as though it had come', async () => {
    let release = () => {};
    const held = new Promise<void>((r) => {
      release = r;
    });
    let paging = false;
    let paged = 0;
    // Opening asks for the first window twice, fast then exact. Only what comes after that is
    // the paging this is about.
    serve((_start) => holding(paging ? WANT : 0), () => {
      if (paging) paged += 1;
    });
    const inner = invoke.getMockImplementation();
    invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === 'graph_frame' && paging) await held;
      return inner?.(cmd, args);
    });

    const graph = new GraphState();
    await graph.open('/kernel');
    paging = true;

    const first = graph.ensureRows(WANT, WANT);
    const second = graph.ensureRows(WANT, WANT);
    let answered = false;
    void second.then(() => {
      answered = true;
    });

    await new Promise((r) => setTimeout(r, 0));
    expect(answered, 'the second ask must not answer before the rows are there').toBe(false);

    release();
    await Promise.all([first, second]);
    expect(paged, 'and it must not fetch the same window twice').toBe(1);
    expect(graph.frame?.startRow).toBe(WANT);
  });

  /**
   * The competing request. Scrolling to a row asks for its own window, and whichever request
   * lands last is the one kept — so asking once could return with a neighbour's frame in place
   * and the rows still missing. The caller has no way to tell, and selected nothing.
   *
   * Modelled by a first answer that does not hold what was asked for, which is what the caller
   * sees when somebody else's frame wins.
   */
  it('keeps asking until the rows it was asked for are the ones in hand', async () => {
    let paging = false;
    let paged = 0;
    serve((_start) => holding(paging && paged > 1 ? WANT : 0), () => {
      if (paging) paged += 1;
    });
    const graph = new GraphState();
    await graph.open('/kernel');
    paging = true;

    await graph.ensureRows(WANT, WANT);

    expect(graph.frame?.startRow, 'the rows asked for are the ones in hand').toBe(WANT);
    expect(paged, 'it asked again rather than giving up').toBeGreaterThan(1);
  });

  it('gives up rather than asking for ever', async () => {
    let paged = 0;
    serve(() => holding(0), () => {
      paged += 1;
    });
    const graph = new GraphState();
    await graph.open('/kernel');

    await graph.ensureRows(WANT, WANT);
    expect(paged).toBeLessThan(8);
  });
});
