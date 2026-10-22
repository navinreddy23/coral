import { invoke } from './invoke';
import { toArrayBuffer } from './bytes';
import type { BlobSide } from './types';

/**
 * A file's bytes on one side of a change, or null where that side has no such file.
 *
 * The engine answers an absent side with no bytes at all, which no image can be.
 */
export async function fileBlob(path: string, side: BlobSide, file: string): Promise<ArrayBuffer | null> {
  const raw = toArrayBuffer(await invoke<unknown>('file_blob', { path, side, file }));
  return raw.byteLength === 0 ? null : raw;
}
