import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import {
  decodeFrame,
  FrameError,
  FrameFlag,
  hasFlag,
  oidOf,
  parentLanesOf,
  RowFlag,
  Section,
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
