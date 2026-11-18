/**
 * Which grammar colours a file, by its name.
 *
 * The names are highlight.js's own. Only the grammars the worker registers are named here, and a
 * file none of them claims is drawn plain: a guess from the contents would colour a log file as
 * whatever it happened to resemble.
 */
export type Grammar =
  | 'bash'
  | 'c'
  | 'cmake'
  | 'cpp'
  | 'csharp'
  | 'css'
  | 'diff'
  | 'go'
  | 'ini'
  | 'java'
  | 'javascript'
  | 'json'
  | 'kotlin'
  | 'lua'
  | 'makefile'
  | 'markdown'
  | 'php'
  | 'python'
  | 'ruby'
  | 'rust'
  | 'sql'
  | 'swift'
  | 'typescript'
  | 'xml'
  | 'yaml';

const BY_EXTENSION: Record<string, Grammar> = {
  sh: 'bash', bash: 'bash', zsh: 'bash',
  c: 'c', h: 'c',
  cmake: 'cmake',
  cc: 'cpp', cpp: 'cpp', cxx: 'cpp', hh: 'cpp', hpp: 'cpp', hxx: 'cpp',
  cs: 'csharp',
  css: 'css',
  diff: 'diff', patch: 'diff',
  go: 'go',
  // highlight.js reads TOML with its INI grammar, under that alias.
  toml: 'ini', ini: 'ini',
  java: 'java',
  js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript',
  json: 'json',
  kt: 'kotlin', kts: 'kotlin',
  lua: 'lua',
  mk: 'makefile',
  md: 'markdown', markdown: 'markdown',
  php: 'php',
  py: 'python', pyi: 'python',
  rb: 'ruby',
  rs: 'rust',
  sql: 'sql',
  swift: 'swift',
  ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript',
  xml: 'xml', html: 'xml', htm: 'xml', svg: 'xml', svelte: 'xml', vue: 'xml',
  yml: 'yaml', yaml: 'yaml',
};

/** Names that carry no extension, or whose extension says nothing. */
const BY_NAME: Record<string, Grammar> = {
  'CMakeLists.txt': 'cmake',
  'Cargo.lock': 'ini',
  Gemfile: 'ruby',
  GNUmakefile: 'makefile',
  Kbuild: 'makefile',
  Makefile: 'makefile',
  makefile: 'makefile',
  Rakefile: 'ruby',
};

/** The grammar for a repository path, or null to draw it plain. */
export function grammarOf(path: string): Grammar | null {
  const name = path.slice(path.lastIndexOf('/') + 1);
  const named = BY_NAME[name];
  if (named !== undefined) return named;
  const dot = name.lastIndexOf('.');
  // A leading dot is a hidden file's name, not an extension: `.bashrc` has none.
  if (dot <= 0) return null;
  return BY_EXTENSION[name.slice(dot + 1).toLowerCase()] ?? null;
}
