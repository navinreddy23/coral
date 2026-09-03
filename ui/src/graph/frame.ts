/**
 * Decoder for the binary graph frame the Rust engine sends.
 *
 * Rows are kept as typed-array views over one ArrayBuffer and are never turned into JavaScript
 * objects: 1.4M row objects would cost hundreds of megabytes in the webview on their own.
 */

export const MAGIC = 0x474c5243; // "CRLG"
export const VERSION = 1;

export const Section = {
  Lane: 1,
  Flags: 2,
  Time: 3,
  ParentStart: 4,
  ParentLane: 5,
  Oid: 6,
  Open: 7,
} as const;

/** Per-row flags, matching `graph::store::flags` in Rust. */
export const RowFlag = {
  Merge: 1 << 0,
  Root: 1 << 1,
  Boundary: 1 << 2,
  Provisional: 1 << 3,
} as const;

/** Frame-level flags, matching `graph::wire::frame` in Rust. */
export const FrameFlag = {
  Final: 1 << 0,
  Provisional: 1 << 1,
} as const;

export interface Frame {
  startRow: number;
  rowCount: number;
  totalRows: number;
  flags: number;
  hashLen: number;
  lanes: Uint16Array;
  rowFlags: Uint8Array;
  times: Float64Array;
  parentStart: Uint32Array;
  parentLanes: Uint16Array;
  oids: Uint8Array;
  /**
   * Per row, a bitmask of the lanes carrying an edge into it from above.
   *
   * The renderer only ever holds a window of rows, so it cannot see the commit that opened a
   * long-running lane. Without this the lane goes undrawn for its whole span and the graph
   * appears to break apart between merges.
   */
  open: Uint32Array;
}

/** magic(4) version(2) sections(2) start(4) rows(4) total(4) flags(1) hashLen(1) + 2 pad. */
const HEADER_BYTES = 24;
const SECTION_BYTES = 16;

export class FrameError extends Error {}

/**
 * Reads a frame. Views alias the input buffer, so it must not be reused.
 */
export function decodeFrame(buffer: ArrayBuffer): Frame {
  if (buffer.byteLength < HEADER_BYTES) {
    throw new FrameError('frame shorter than its header');
  }
  const view = new DataView(buffer);
  if (view.getUint32(0, true) !== MAGIC) throw new FrameError('bad magic');

  const version = view.getUint16(4, true);
  if (version !== VERSION) throw new FrameError(`unsupported frame version ${version}`);

  const sectionCount = view.getUint16(6, true);
  const frame: Frame = {
    startRow: view.getUint32(8, true),
    rowCount: view.getUint32(12, true),
    totalRows: view.getUint32(16, true),
    flags: view.getUint8(20),
    hashLen: view.getUint8(21),
    lanes: new Uint16Array(0),
    rowFlags: new Uint8Array(0),
    times: new Float64Array(0),
    parentStart: new Uint32Array(0),
    parentLanes: new Uint16Array(0),
    oids: new Uint8Array(0),
    open: new Uint32Array(0),
  };

  for (let i = 0; i < sectionCount; i++) {
    const base = HEADER_BYTES + i * SECTION_BYTES;
    if (base + SECTION_BYTES > buffer.byteLength) {
      throw new FrameError('section directory runs past the frame');
    }
    const kind = view.getUint32(base, true);
    const offset = view.getUint32(base + 4, true);
    const byteLen = view.getUint32(base + 8, true);
    const elements = view.getUint32(base + 12, true);

    if (offset + byteLen > buffer.byteLength) {
      throw new FrameError(`section ${kind} runs past the frame`);
    }
    // A typed-array view over a misaligned offset throws; the encoder pads for this reason.
    if (offset % 8 !== 0) throw new FrameError(`section ${kind} is not 8-byte aligned`);

    switch (kind) {
      case Section.Lane:
        frame.lanes = new Uint16Array(buffer, offset, elements);
        break;
      case Section.Flags:
        frame.rowFlags = new Uint8Array(buffer, offset, elements);
        break;
      case Section.Time:
        frame.times = new Float64Array(buffer, offset, elements);
        break;
      case Section.ParentStart:
        frame.parentStart = new Uint32Array(buffer, offset, elements);
        break;
      case Section.ParentLane:
        frame.parentLanes = new Uint16Array(buffer, offset, elements);
        break;
      case Section.Oid:
        frame.oids = new Uint8Array(buffer, offset, byteLen);
        break;
      case Section.Open:
        frame.open = new Uint32Array(buffer, offset, elements);
        break;
      default:
        throw new FrameError(`unknown section ${kind}`);
    }
  }
  return frame;
}

/** The lanes this row's edges run to, as a view rather than a copy. */
export function parentLanesOf(frame: Frame, row: number): Uint16Array {
  const start = frame.parentStart[row];
  const end = frame.parentStart[row + 1];
  if (start === undefined || end === undefined) return new Uint16Array(0);
  return frame.parentLanes.subarray(start, end);
}

/** The object id of a row, as lowercase hex. */
export function oidOf(frame: Frame, row: number): string {
  const start = row * frame.hashLen;
  const bytes = frame.oids.subarray(start, start + frame.hashLen);
  let out = '';
  for (const b of bytes) out += b.toString(16).padStart(2, '0');
  return out;
}

export function hasFlag(value: number, flag: number): boolean {
  return (value & flag) !== 0;
}

/** Whether lane `lane` carries an edge into `row` from the rows above it. */
export function laneOpenAt(frame: Frame, row: number, lane: number): boolean {
  if (lane >= 32) return false;
  const mask = frame.open[row];
  return mask !== undefined && (mask & (1 << lane)) !== 0;
}
