/**
 * What went wrong, in words a person can act on.
 *
 * A command that fails in Rust rejects with the serialized `IpcError` — a plain
 * `{ code, message }` object, not an `Error`. Every failure path in the window used to read
 * `e instanceof Error ? e.message : String(e)`, and `String` of a plain object is
 * `[object Object]`. That is what the user was shown for every engine failure there is: a
 * terminal that would not start, a push that was rejected, a repository that would not open.
 */
export function messageOf(cause: unknown): string {
  if (typeof cause === 'string') return cause;
  if (cause instanceof Error) return cause.message;

  if (typeof cause === 'object' && cause !== null) {
    const shape = cause as { message?: unknown; code?: unknown };
    if (typeof shape.message === 'string' && shape.message !== '') return shape.message;
    // No message, but a code is still better than nothing: it is what the engine switches on
    // and what a bug report can be searched for.
    if (typeof shape.code === 'string' && shape.code !== '') return shape.code;
    // Anything else at least reaches the user as its own shape rather than as a type name.
    try {
      return JSON.stringify(cause);
    } catch {
      // Circular, which JSON.stringify refuses. Fall through.
    }
  }
  return String(cause);
}

/**
 * The engine's stable tag for a failure, when there is one.
 *
 * Never shown; this is for deciding what to do, since the message is prose and may be reworded.
 */
export function codeOf(cause: unknown): string | null {
  if (typeof cause === 'object' && cause !== null) {
    const shape = cause as { code?: unknown };
    if (typeof shape.code === 'string' && shape.code !== '') return shape.code;
  }
  return null;
}
