import type { Span } from '../diff/word';
import type { Category, Token } from './token';

/** A stretch of a line drawn one way: one colour, marked as changed or not. */
export interface Segment {
  text: string;
  marked: boolean;
  category: Category | null;
}

/**
 * A line cut wherever its colour or its changed-word mark changes.
 *
 * The two come from different places and do not agree on where words end: a changed word can
 * cut through the middle of a string. Each segment carries both, so the mark is drawn over the
 * colour rather than instead of it.
 */
export function segmentsOf(text: string, tokens: readonly Token[] | null, spans: readonly Span[] | null): Segment[] {
  const cuts = new Set<number>([0, text.length]);
  const marked: [number, number][] = [];
  let at = 0;
  for (const span of spans ?? []) {
    const end = at + span.text.length;
    cuts.add(at);
    cuts.add(end);
    if (span.marked) marked.push([at, end]);
    at = end;
  }
  const coloured = (tokens ?? []).filter((t) => t.from < text.length);
  for (const t of coloured) {
    cuts.add(t.from);
    cuts.add(Math.min(t.to, text.length));
  }
  const edges = [...cuts].filter((n) => n <= text.length).sort((a, b) => a - b);

  const out: Segment[] = [];
  for (let i = 0; i + 1 < edges.length; i++) {
    const from = edges[i] ?? 0;
    const to = edges[i + 1] ?? from;
    if (to <= from) continue;
    const piece = {
      text: text.slice(from, to),
      marked: marked.some(([a, b]) => a <= from && to <= b),
      category: coloured.find((t) => t.from <= from && to <= t.to)?.category ?? null,
    };
    // A word boundary with the same colour and mark on both sides is no boundary at all.
    const last = out[out.length - 1];
    if (last !== undefined && last.marked === piece.marked && last.category === piece.category) {
      last.text += piece.text;
    } else {
      out.push(piece);
    }
  }
  return out;
}
