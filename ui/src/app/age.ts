/**
 * How long ago something happened, in the shortest form that still says it.
 *
 * Hours for the last day, days for the last year, years beyond that. A commit list is read
 * down a column, so what matters is the order of magnitude, not the minute.
 */
export function shortAge(seconds: number): string {
  const delta = Date.now() / 1000 - seconds;
  const hours = delta / 3600;
  if (hours < 24) return `${Math.max(1, Math.round(hours))}h`;
  const days = hours / 24;
  if (days < 365) return `${Math.round(days)}d`;
  return `${(days / 365).toFixed(1)}y`;
}
