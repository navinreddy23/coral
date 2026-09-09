import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import {
  covers,
  decodeFrame,
  frameStartFor,
  localRow,
  ROWS_PER_FRAME,
  FrameError,
  FrameFlag,
  hasFlag,
  oidOf,
  rowOfOid,
  parentLanesOf,
  RowFlag,
  widestLane,
  Section,
  type Frame,
} from '../src/graph/frame';

/**
 * The same bytes `crates/coral-core/tests/wire.rs` writes. Encoder and decoder living in one
 * language prove only self-consistency; this file is what makes a layout change fail on both
 * sides rather than silently on one.
 */
function golden(): ArrayBuffer {
  const path = fileURLToPath(new URL('./fixtures/frame.bin', import.meta.url));
  const bytes = readFileSync(path);
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

describe('decodeFrame', () => {
  it('reads the header the Rust encoder wrote', () => {
    const frame = decodeFrame(golden());

    expect(frame.startRow).toBe(0);
    expect(frame.rowCount).toBe(5);
    expect(frame.totalRows).toBe(5);
    expect(frame.hashLen).toBe(20);
  });

  it('exposes every section as a typed array of the right length', () => {
    const frame = decodeFrame(golden());

    expect(frame.lanes.length).toBe(frame.rowCount);
    expect(frame.rowFlags.length).toBe(frame.rowCount);
    expect(frame.times.length).toBe(frame.rowCount);
    expect(frame.parentStart.length).toBe(frame.rowCount + 1);
    expect(frame.oids.length).toBe(frame.rowCount * frame.hashLen);
  });

  it('agrees with Rust about which rows are merges and roots', () => {
    const frame = decodeFrame(golden());

    const merges = [...frame.rowFlags].filter((f) => hasFlag(f, RowFlag.Merge)).length;
    const roots = [...frame.rowFlags].filter((f) => hasFlag(f, RowFlag.Root)).length;

    expect(merges).toBe(1);
    expect(roots).toBe(2);
  });

  it('marks a completed topological walk final and not provisional', () => {
    const frame = decodeFrame(golden());

    expect(hasFlag(frame.flags, FrameFlag.Final)).toBe(true);
    expect(hasFlag(frame.flags, FrameFlag.Provisional)).toBe(false);
  });

  it('resolves each row edges to lanes through the CSR offsets', () => {
    const frame = decodeFrame(golden());

    let edges = 0;
    for (let row = 0; row < frame.rowCount; row++) {
      const lanes = parentLanesOf(frame, row);
      edges += lanes.length;
      // A merge has two parents; a root has none.
      if (hasFlag(frame.rowFlags[row]!, RowFlag.Merge)) expect(lanes.length).toBe(2);
      if (hasFlag(frame.rowFlags[row]!, RowFlag.Root)) expect(lanes.length).toBe(0);
    }
    expect(edges).toBe(frame.parentLanes.length);
    expect(frame.parentStart[frame.rowCount]).toBe(edges);
  });

  it('renders object ids as hex of the right width', () => {
    const frame = decodeFrame(golden());

    for (let row = 0; row < frame.rowCount; row++) {
      const oid = oidOf(frame, row);
      expect(oid).toHaveLength(40);
      expect(oid).toMatch(/^[0-9a-f]{40}$/);
    }
    // Distinct commits have distinct ids.
    const all = new Set(Array.from({ length: frame.rowCount }, (_, r) => oidOf(frame, r)));
    expect(all.size).toBe(frame.rowCount);
  });

  it('reads times as seconds that fit exactly in a double', () => {
    const frame = decodeFrame(golden());

    for (const t of frame.times) {
      expect(Number.isInteger(t)).toBe(true);
      expect(t).toBeGreaterThan(0);
      expect(t).toBeLessThan(4102444800); // well before the year 2100
    }
  });

  it('rejects a frame it cannot trust rather than misreading it', () => {
    expect(() => decodeFrame(new ArrayBuffer(4))).toThrow(FrameError);

    const badMagic = golden();
    new DataView(badMagic).setUint32(0, 0xdeadbeef, true);
    expect(() => decodeFrame(badMagic)).toThrow(/magic/);

    const badVersion = golden();
    new DataView(badVersion).setUint16(4, 99, true);
    expect(() => decodeFrame(badVersion)).toThrow(/version/);

    const badSection = golden();
    new DataView(badSection).setUint32(24, 200, true);
    expect(() => decodeFrame(badSection)).toThrow(/unknown section/);
  });

  it('keeps the section ids that are part of the contract', () => {
    expect(Section.Lane).toBe(1);
    expect(Section.Flags).toBe(2);
    expect(Section.Time).toBe(3);
    expect(Section.ParentStart).toBe(4);
    expect(Section.ParentLane).toBe(5);
    expect(Section.Oid).toBe(6);
  });
});

describe('paging', () => {
  function frameAt(startRow: number, rowCount: number, totalRows: number): Frame {
    return {
      startRow,
      rowCount,
      totalRows,
      flags: 0,
      hashLen: 20,
      lanes: new Uint16Array(rowCount),
      rowFlags: new Uint8Array(rowCount),
      times: new Float64Array(rowCount),
      parentStart: new Uint32Array(rowCount + 1),
      parentLanes: new Uint16Array(0),
      oids: new Uint8Array(rowCount * 20),
      open: new Uint32Array(rowCount),
    };
  }

  const kernel = 1_481_528;

  it('knows which rows a frame holds', () => {
    // Expressed against the constant, not a number: the frame size is a tuning decision and
    // the boundaries have to follow it.
    const start = ROWS_PER_FRAME;
    const frame = frameAt(start, ROWS_PER_FRAME, kernel);
    expect(covers(frame, start, start + ROWS_PER_FRAME - 1)).toBe(true);
    expect(covers(frame, start - 1, start + 1)).toBe(false);
    expect(covers(frame, start + ROWS_PER_FRAME - 1, start + ROWS_PER_FRAME)).toBe(false);
    expect(covers(null, 0, 0)).toBe(false);
  });

  it('places a frame with room to scroll back', () => {
    const start = frameStartFor(900_000, kernel);
    expect(start).toBeLessThan(900_000);
    expect(start + ROWS_PER_FRAME).toBeGreaterThan(900_000);
  });

  it('holds an ordinary repository whole, so it never pages at all', () => {
    // Nearly every repository is smaller than one frame; paging is the exception.
    const modest = 40_000;
    expect(frameStartFor(modest - 1, modest)).toBe(0);
    expect(covers(frameAt(0, modest, modest), 0, modest - 1)).toBe(true);
  });

  it('never asks for rows past the end of the graph', () => {
    // The engine clamps, but a frame that starts past the end comes back short and the window
    // would then fall outside it on every scroll, refetching forever.
    const start = frameStartFor(kernel - 1, kernel);
    expect(start + ROWS_PER_FRAME).toBeLessThanOrEqual(kernel);
    expect(covers(frameAt(start, ROWS_PER_FRAME, kernel), kernel - 1, kernel - 1)).toBe(true);
  });

  it('starts at zero near the top, and for a graph shorter than one frame', () => {
    expect(frameStartFor(10, kernel)).toBe(0);
    expect(frameStartFor(5, 100)).toBe(0);
    expect(frameStartFor(99, 100)).toBe(0);
  });

  it('sizes the lane column on edges and pass-throughs, not just on the nodes', () => {
    const frame = frameAt(0, 8, 8);
    frame.lanes.set([0, 1, 0, 2, 0, 0, 0, 0]);
    // Row 4 has no node out at lane 5, but an edge runs down to it and the column has to
    // cover it or the line is drawn off the canvas.
    frame.parentStart.set([0, 0, 0, 0, 0, 0, 1, 1, 1]);
    frame.parentLanes = new Uint16Array([5]);
    // Row 7 has lane 9 crossing it with nothing of its own in view.
    frame.open[7] = 1 << 9;

    expect(widestLane(frame, [0, 1, 2])).toBe(1);
    expect(widestLane(frame, [0, 1, 2, 3])).toBe(2);
    expect(widestLane(frame, [5])).toBe(5);
    expect(widestLane(frame, [7])).toBe(9);
  });

  it('ignores rows the frame does not hold, and has no frame at all', () => {
    // Mid-scroll the window can run past the frame; those rows must not read as lane zero and
    // shrink the column, nor throw.
    const frame = frameAt(1000, 8, 8000);
    frame.lanes.set([3, 0, 0, 0, 0, 0, 0, 0]);
    expect(widestLane(frame, [1000, 5, 999_999])).toBe(3);
    expect(widestLane(null, [0, 1, 2])).toBe(0);
    expect(widestLane(frame, [])).toBe(0);
  });

  it('maps an absolute row into the frame, and refuses one it does not hold', () => {
    const frame = frameAt(4096, ROWS_PER_FRAME, kernel);
    expect(localRow(frame, 4096)).toBe(0);
    expect(localRow(frame, 5000)).toBe(904);
    expect(localRow(frame, 4095)).toBeNull();
    expect(localRow(frame, 900_000)).toBeNull();
  });
});

describe('finding a commit again', () => {
  function frameHolding(startRow: number, oids: string[]): Frame {
    const rowCount = oids.length;
    const bytes = new Uint8Array(rowCount * 20);
    oids.forEach((oid, row) => {
      for (let i = 0; i < 20; i += 1) {
        bytes[row * 20 + i] = Number.parseInt(oid.slice(i * 2, i * 2 + 2), 16);
      }
    });
    return {
      startRow,
      rowCount,
      totalRows: startRow + rowCount + 500,
      flags: 0,
      hashLen: 20,
      lanes: new Uint16Array(rowCount),
      rowFlags: new Uint8Array(rowCount),
      times: new Float64Array(rowCount),
      parentStart: new Uint32Array(rowCount + 1),
      parentLanes: new Uint16Array(0),
      oids: bytes,
      open: new Uint32Array(rowCount),
    };
  }

  const a = 'a'.repeat(40);
  const b = 'b'.repeat(40);
  const c = 'c'.repeat(40);

  it('answers with the absolute row, not the one inside the frame', () => {
    const frame = frameHolding(4096, [a, b, c]);
    expect(rowOfOid(frame, b)).toBe(4097);
  });

  it('answers null for a commit the window does not hold, and for no frame', () => {
    const frame = frameHolding(0, [a, b]);
    expect(rowOfOid(frame, c)).toBeNull();
    expect(rowOfOid(null, a)).toBeNull();
  });

  it('takes an id shorter than the hash, which is how the window writes them', () => {
    const frame = frameHolding(0, [a, b, c]);
    expect(rowOfOid(frame, 'cccccccc')).toBe(2);
  });

  it('refuses an id that is not hex, rather than matching the wrong row', () => {
    const frame = frameHolding(0, [a, b]);
    expect(rowOfOid(frame, 'not a hash')).toBeNull();
    expect(rowOfOid(frame, '')).toBeNull();
  });
});

describe('an id that is only partly a hash', () => {
  it('is refused, because parseInt would read the front of it and stop', () => {
    // `parseInt('1z', 16)` is 1, so a pair-by-pair reading matched `1z…` against `1a…`.
    const bytes = new Uint8Array(20);
    bytes[0] = 0x1a;
    const frame: Frame = {
      startRow: 0,
      rowCount: 1,
      totalRows: 1,
      flags: 0,
      hashLen: 20,
      lanes: new Uint16Array(1),
      rowFlags: new Uint8Array(1),
      times: new Float64Array(1),
      parentStart: new Uint32Array(2),
      parentLanes: new Uint16Array(0),
      oids: bytes,
      open: new Uint32Array(1),
    };
    expect(rowOfOid(frame, '1a')).toBe(0);
    expect(rowOfOid(frame, '1z')).toBeNull();
  });
});
