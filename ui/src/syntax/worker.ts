/**
 * Colours whole files off the window's thread.
 *
 * Highlighting the kernel's largest C files takes longer than a frame, and done on the main
 * thread it would hold up the scroll that asked for it. The grammars are registered one by one
 * rather than through the library's bundle of nearly two hundred, which would be most of a
 * megabyte nobody reads.
 */
import hljs from 'highlight.js/lib/core';
import bash from 'highlight.js/lib/languages/bash';
import c from 'highlight.js/lib/languages/c';
import cmake from 'highlight.js/lib/languages/cmake';
import cpp from 'highlight.js/lib/languages/cpp';
import csharp from 'highlight.js/lib/languages/csharp';
import css from 'highlight.js/lib/languages/css';
import diff from 'highlight.js/lib/languages/diff';
import go from 'highlight.js/lib/languages/go';
import ini from 'highlight.js/lib/languages/ini';
import java from 'highlight.js/lib/languages/java';
import javascript from 'highlight.js/lib/languages/javascript';
import json from 'highlight.js/lib/languages/json';
import kotlin from 'highlight.js/lib/languages/kotlin';
import lua from 'highlight.js/lib/languages/lua';
import makefile from 'highlight.js/lib/languages/makefile';
import markdown from 'highlight.js/lib/languages/markdown';
import php from 'highlight.js/lib/languages/php';
import python from 'highlight.js/lib/languages/python';
import ruby from 'highlight.js/lib/languages/ruby';
import rust from 'highlight.js/lib/languages/rust';
import sql from 'highlight.js/lib/languages/sql';
import swift from 'highlight.js/lib/languages/swift';
import typescript from 'highlight.js/lib/languages/typescript';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';

import type { Grammar } from './grammar';
import { tokensOf, type Token } from './token';

const GRAMMARS: Record<Grammar, Parameters<typeof hljs.registerLanguage>[1]> = {
  bash, c, cmake, cpp, csharp, css, diff, go, ini, java, javascript, json, kotlin, lua, makefile,
  markdown, php, python, ruby, rust, sql, swift, typescript, xml, yaml,
};
for (const [name, grammar] of Object.entries(GRAMMARS)) hljs.registerLanguage(name, grammar);

export interface Ask {
  id: number;
  grammar: Grammar;
  text: string;
}

export interface Answer {
  id: number;
  /** One list of tokens per line, or null where the grammar could not read the file. */
  lines: Token[][] | null;
}

// The DOM's types describe a window, whose `postMessage` wants an origin; a worker's does not.
const scope = self as unknown as {
  onmessage: ((event: MessageEvent<Ask>) => void) | null;
  postMessage: (answer: Answer) => void;
};

scope.onmessage = (event) => {
  const { id, grammar, text } = event.data;
  let lines: Token[][] | null = null;
  try {
    lines = tokensOf(hljs.highlight(text, { language: grammar, ignoreIllegals: true }).value);
  } catch {
    // A grammar that throws on some input draws that file plain, which is what it was before.
  }
  scope.postMessage({ id, lines });
};
