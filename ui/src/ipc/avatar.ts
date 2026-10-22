import { invoke } from './invoke';
import { toArrayBuffer } from './bytes';

/** The picture of whoever writes as `email`, or null when there is none to be had. */
export async function authorAvatar(email: string): Promise<ArrayBuffer | null> {
  const raw = toArrayBuffer(await invoke<unknown>('author_avatar', { email }));
  return raw.byteLength === 0 ? null : raw;
}
