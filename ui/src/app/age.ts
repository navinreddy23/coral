/**
 * How long ago something happened, in the shortest form that still says it.
 *
 * Minutes for the first hour, hours for the first day, days for the first year, years beyond
 * that. A commit list is read down a column, so what matters is the order of magnitude — but
 * not below an hour, where the commit you just recorded sits at the top of the list and the
 * one thing it must not say is that it is an hour old.
 */
export function shortAge(seconds: number): string {
  const delta = Date.now() / 1000 - seconds;
  const minutes = delta / 60;
  if (minutes < 1) return 'now';
  if (minutes < 60) return `${Math.round(minutes)}m`;
  const hours = minutes / 60;
  if (hours < 24) return `${Math.round(hours)}h`;
  const days = hours / 24;
  if (days < 365) return `${Math.round(days)}d`;
  return `${(days / 365).toFixed(1)}y`;
}
