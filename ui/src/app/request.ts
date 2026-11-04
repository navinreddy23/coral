import type { Host } from '../ipc/commands';

/**
 * How a pull or merge request is quoted on its host. GitLab keeps `#` for issues and writes a
 * merge request as `!42`, so `#42` there names something else entirely.
 */
export function quoteRequest(kind: Host['kind'] | undefined, number: number): string {
  return `${kind === 'gitlab' ? '!' : '#'}${number}`;
}
