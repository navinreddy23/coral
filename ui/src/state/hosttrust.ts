import { sshTrustHostKey } from '../ipc/commands';
import { hostKeyOf } from '../ipc/error';
import type { HostKey } from '../ipc/types';

/** Asks the user whether to trust a host ssh has not connected to before. */
export type AskTrust = (key: HostKey) => Promise<boolean>;

/** Never trusts anything: what a caller that cannot ask gets. */
export const neverTrust: AskTrust = () => Promise.resolve(false);

/**
 * After `failure`, when ssh stopped at a host it has no record of, asks and runs `op` again.
 *
 * Called only once something has failed, so an operation that succeeds takes exactly as long
 * to settle as it did before: a wrapper around every call delayed the end of each by several
 * turns, and a redo pressed straight after an undo found the undo still running.
 *
 * Each key is asked about once. The same key coming back after it was trusted means this ssh
 * cannot be answered through Coral, one older than 8.4 for instance, and asking again would go
 * round for ever; a second new host, a submodule's, is a key of its own and is asked about.
 */
export async function retryAfterTrust<T>(
  failure: unknown,
  op: () => Promise<T>,
  ask: AskTrust,
): Promise<T> {
  const asked = new Set<string>();
  let cause = failure;
  for (;;) {
    const key = hostKeyOf(cause);
    if (key === null) throw cause;
    const id = `${key.host} ${key.fingerprint}`;
    if (asked.has(id) || !(await ask(key))) throw cause;
    asked.add(id);
    await sshTrustHostKey(key.host, key.fingerprint);
    try {
      return await op();
    } catch (next) {
      cause = next;
    }
  }
}
