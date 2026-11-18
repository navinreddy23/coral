import type { Grammar } from './grammar';
import type { Token } from './token';
import type { Answer, Ask } from './worker';

let worker: Worker | null = null;
/** Set once the worker has failed to load, so it is not started again for every file. */
let failed = false;
let next = 0;
const waiting = new Map<number, (lines: Token[][] | null) => void>();

/**
 * Colours a whole file in the worker: one list of tokens per line, or null to draw it plain.
 *
 * Never rejects. A worker that cannot start, or that falls over, leaves code uncoloured, which
 * is how every file was drawn before there was a worker.
 */
export function highlight(grammar: Grammar, text: string): Promise<Token[][] | null> {
  const w = worker ?? start();
  if (w === null) return Promise.resolve(null);
  const id = ++next;
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    const ask: Ask = { id, grammar, text };
    w.postMessage(ask);
  });
}

function start(): Worker | null {
  if (failed) return null;
  try {
    worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
  } catch {
    failed = true;
    return null;
  }
  worker.onmessage = (event: MessageEvent<Answer>) => {
    const resolve = waiting.get(event.data.id);
    waiting.delete(event.data.id);
    resolve?.(event.data.lines);
  };
  worker.onerror = () => {
    for (const resolve of waiting.values()) resolve(null);
    waiting.clear();
    worker = null;
    failed = true;
  };
  return worker;
}
