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
