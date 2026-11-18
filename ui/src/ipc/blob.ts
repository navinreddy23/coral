import { save } from '@tauri-apps/plugin-dialog';
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

/** Asks where to save a file, suggesting its own name; null when the dialog was cancelled. */
export async function chooseSavePath(name: string): Promise<string | null> {
  return (await save({ defaultPath: name })) ?? null;
}

/** Writes a file as one side of a change has it to `to`, straight from git. */
export function saveBlob(path: string, side: BlobSide, file: string, to: string): Promise<void> {
  return invoke<void>('save_blob', { path, side, file, to });
}
