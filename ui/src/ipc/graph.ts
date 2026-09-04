import { invoke } from './invoke';

import { decodeFrame, type Frame } from '../graph/frame';
import type { CommitMeta } from './types';

/**
 * Fetches one binary frame of graph rows.
 *
 * `invoke` returns an ArrayBuffer when the command answered with raw bytes and the custom
 * protocol is working. On the postMessage fallback it returns a plain number array instead,
 * which is why {@link checkBinaryTransport} runs first.
 */
export async function graphFrame(
  path: string,
  startRow: number,
  firstPaint = false,
): Promise<Frame> {
  const raw = await invoke<ArrayBuffer>('graph_frame', { path, startRow, firstPaint });
  return decodeFrame(toArrayBuffer(raw));
}

/** Author and summary for rows on screen, fetched separately from the frame. */
export async function rowMetadata(
  path: string,
  startRow: number,
  count: number,
  provisional: boolean,
): Promise<CommitMeta[]> {
  return invoke<CommitMeta[]>('row_metadata', { path, startRow, count, provisional });
}

export interface TransportCheck {
  binary: boolean;
  detail: string;
}

/**
 * Confirms the binary IPC path is live.
 *
 * Tauri's JavaScript sets an internal flag permanently if the custom-protocol fetch ever
 * throws, and thereafter sends every response through postMessage, where a byte array is
 * serialized as JSON numbers. That is roughly a hundred times slower and reports no error at
 * all, so the graph would simply feel broken. Checking once at startup makes it loud.
 */
export async function checkBinaryTransport(): Promise<TransportCheck> {
  const raw = await invoke<ArrayBuffer>('binary_self_test');

  if (Array.isArray(raw)) {
    return {
      binary: false,
      detail:
        'IPC returned a JSON array instead of bytes: Tauri fell back to postMessage, and the ' +
        'graph will be far slower than it should be.',
    };
  }

  const bytes = new Uint8Array(toArrayBuffer(raw));
  if (bytes.length !== 4096) {
    return { binary: false, detail: `self-test returned ${bytes.length} bytes, expected 4096` };
  }
  for (let i = 0; i < bytes.length; i++) {
    if (bytes[i] !== i % 251) {
      return { binary: false, detail: `self-test byte ${i} was ${bytes[i]}, expected ${i % 251}` };
    }
  }
  return { binary: true, detail: 'binary IPC confirmed' };
}

/** Normalises what `invoke` hands back for a raw-bytes command. */
function toArrayBuffer(raw: unknown): ArrayBuffer {
  if (raw instanceof ArrayBuffer) return raw;
  if (ArrayBuffer.isView(raw)) {
    const view = raw as ArrayBufferView;
    return view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength) as ArrayBuffer;
  }
  if (Array.isArray(raw)) return new Uint8Array(raw as number[]).buffer;
  throw new TypeError('graph command did not return bytes');
}

/**
 * Forgets the walk held for a repository, so the next frame walks it again.
 *
 * The engine keeps one walk per repository, which is what makes scrolling free. Asking for the
 * graph again has to say so, or a commit made since is simply not there.
 */
export function graphRewalk(path: string): Promise<void> {
  return invoke<void>('graph_rewalk', { path });
}
