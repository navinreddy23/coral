/**
 * How well a command label answers a palette query.
 *
 * Subsequence matching, so "cob" finds "Checkout branch". Ranked by how tightly the match
 * packs — a run of adjacent characters beats the same letters scattered across the label —
 * with an earlier match breaking ties, which is what makes a short query land on the obvious
 * answer rather than on whichever command happens to be first.
 *
 * Lower is better. `null` when the label does not contain the query at all.
 */
export function rankCommand(text: string, query: string): number | null {
  let at = 0;
  let gaps = 0;
  let previous = -1;
  for (const ch of query) {
    const found = text.indexOf(ch, at);
    if (found < 0) return null;
    if (previous >= 0) gaps += found - previous - 1;
    previous = found;
    at = found + 1;
  }
  return gaps * 100 + (previous - query.length);
}
