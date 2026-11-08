/**
 * What a stretch of code is, as far as its colour goes.
 *
 * Six kinds and no more. A highlighter knows forty scopes, and a diff coloured forty ways is a
 * diff whose additions and removals no longer stand out from the code they are made of.
 */
export type Category = 'keyword' | 'string' | 'comment' | 'constant' | 'type' | 'meta';

/** One coloured stretch of a line, by offsets into the line's text. */
export interface Token {
  from: number;
  to: number;
  category: Category;
}

/**
 * The six kinds, from highlight.js's scope names.
 *
 * `null` stops a scope from taking its parent's colour: the code inside a template string's
 * `${}` is code, not string. A scope missing here takes the colour of the one around it.
 */
const CATEGORY_OF: Record<string, Category | null> = {
  keyword: 'keyword',
  'selector-tag': 'keyword',
  string: 'string',
  regexp: 'string',
  char: 'string',
  symbol: 'string',
  comment: 'comment',
  quote: 'comment',
  doctag: 'comment',
  number: 'constant',
  literal: 'constant',
  type: 'type',
  title: 'type',
  built_in: 'type',
  class: 'type',
  meta: 'meta',
  attr: 'meta',
  attribute: 'meta',
  tag: 'meta',
  name: 'meta',
  section: 'meta',
  'selector-id': 'meta',
  'selector-class': 'meta',
  'template-tag': 'meta',
  subst: null,
};

/**
 * The tokens on each line of highlight.js's HTML for a whole file.
 *
 * Read from the HTML because that is the answer the library documents; its tree is private.
 * The markup is only ever spans and five entities, and a span can run across lines — a block
 * comment does — so lines are cut here rather than by the library.
 */
export function tokensOf(html: string): Token[][] {
  const lines: Token[][] = [[]];
  const open: (Category | null | undefined)[] = [];
  let column = 0;
  let at = 0;
  const run = (length: number) => {
    const category = innermost(open);
    const line = lines[lines.length - 1];
    if (category !== null && line !== undefined) add(line, column, column + length, category);
    column += length;
  };
  while (at < html.length) {
    const c = html[at];
    if (c === '<') {
      const end = html.indexOf('>', at);
      const tag = html.slice(at + 1, end);
      if (tag.startsWith('/')) open.pop();
      else open.push(scopeOf(tag));
      at = end + 1;
    } else if (c === '\n') {
      lines.push([]);
      column = 0;
      at += 1;
    } else if (c === '&') {
      // Every entity the library writes stands for one character.
      run(1);
      at = html.indexOf(';', at) + 1;
    } else {
      let end = at;
      while (end < html.length && html[end] !== '<' && html[end] !== '&' && html[end] !== '\n') end++;
      run(end - at);
      at = end;
    }
  }
  return lines;
}

/** The colour a span's class gives, from its first `hljs-` name: `hljs-title function_`. */
function scopeOf(tag: string): Category | null | undefined {
  const name = /class="hljs-([\w-]+)/u.exec(tag)?.[1];
  return name === undefined ? undefined : CATEGORY_OF[name];
}

function innermost(open: readonly (Category | null | undefined)[]): Category | null {
  for (let i = open.length - 1; i >= 0; i--) {
    const scope = open[i];
    if (scope !== undefined) return scope;
  }
  return null;
}

function add(line: Token[], from: number, to: number, category: Category): void {
  if (to <= from) return;
  const last = line[line.length - 1];
  if (last !== undefined && last.to === from && last.category === category) last.to = to;
  else line.push({ from, to, category });
}
