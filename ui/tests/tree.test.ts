import { describe, expect, it } from 'vitest';

import { buildTree, filesIn, type TreeDir, type TreeNode } from '../src/diff/tree';
import type { ChangedFile } from '../src/ipc/types';

function file(path: string): ChangedFile {
  return { path, oldPath: null, change: 'modified' };
}

/** The tree as indented text, which is a far easier diff to read than nested objects. */
function render(nodes: TreeNode[], depth = 0): string {
  return nodes
    .map((n) =>
      n.kind === 'dir'
        ? `${'  '.repeat(depth)}${n.name}/\n${render(n.children, depth + 1)}`
        : `${'  '.repeat(depth)}${n.name}\n`,
    )
    .join('');
}

describe('buildTree', () => {
  it('groups files under their directories', () => {
    const tree = buildTree([file('src/a.rs'), file('src/b.rs'), file('README.md')]);
    expect(render(tree)).toBe('src/\n  a.rs\n  b.rs\nREADME.md\n');
  });

  it('collapses a chain of directories holding nothing else', () => {
    // Five levels of indent to show one file is what makes a deep tree unusable.
    const tree = buildTree([file('drivers/net/ethernet/intel/ice/ice_main.c')]);
    expect(render(tree)).toBe('drivers/net/ethernet/intel/ice/\n  ice_main.c\n');
  });

  it('stops collapsing where a directory holds more than one thing', () => {
    const tree = buildTree([file('a/b/c/one.txt'), file('a/b/d/two.txt')]);
    expect(render(tree)).toBe('a/b/\n  c/\n    one.txt\n  d/\n    two.txt\n');
  });

  it('does not collapse a directory that also holds a file', () => {
    const tree = buildTree([file('a/b/one.txt'), file('a/note.md')]);
    expect(render(tree)).toBe('a/\n  b/\n    one.txt\n  note.md\n');
  });

  it('puts directories before files and orders each by name', () => {
    const tree = buildTree([file('z.txt'), file('a.txt'), file('sub/x.txt')]);
    expect(render(tree)).toBe('sub/\n  x.txt\na.txt\nz.txt\n');
  });

  it('keeps the full path on a collapsed directory, not the display name', () => {
    const tree = buildTree([file('a/b/c/one.txt')]);
    const dir = tree[0] as TreeDir;
    expect(dir.name).toBe('a/b/c');
    expect(dir.path).toBe('a/b/c');
  });

  it('splits on slash whatever the host separator is', () => {
    // git reports slash-separated paths on every platform, backslash included as a filename.
    const tree = buildTree([file('dir/a\\b.txt')]);
    expect(render(tree)).toBe('dir/\n  a\\b.txt\n');
  });

  it('has nothing to show for a commit that changed nothing', () => {
    expect(buildTree([])).toEqual([]);
  });
});

describe('a tree of anything with a path', () => {
  it('carries whatever the caller put in, not only a commit file', () => {
    // The staging panel arranges status entries with the same code, which is the point of it
    // being generic rather than typed to one shape.
    const entries = [
      { path: 'src/app/App.svelte', index: 'unmodified' },
      { path: 'src/app/Menu.svelte', index: 'modified' },
    ];
    const nodes = buildTree(entries);
    expect(nodes).toHaveLength(1);
    const dir = nodes[0];
    expect(dir?.kind).toBe('dir');
    if (dir?.kind !== 'dir') throw new Error('expected a directory');
    expect(dir.children.map((c) => (c.kind === 'file' ? c.item.index : null))).toEqual([
      'unmodified',
      'modified',
    ]);
  });

  it('reaches every file under a node, which is what a directory action needs', () => {
    const nodes = buildTree([
      { path: 'a/b/one.txt' },
      { path: 'a/c/two.txt' },
      { path: 'top.txt' },
    ]);
    const a = nodes.find((n) => n.kind === 'dir');
    expect(a).toBeDefined();
    if (!a) throw new Error('no directory');
    expect(filesIn(a).map((f) => f.path)).toEqual(['a/b/one.txt', 'a/c/two.txt']);
  });

  it('counts a single file node as itself', () => {
    const nodes = buildTree([{ path: 'only.txt' }]);
    expect(filesIn(nodes[0]!).map((f) => f.path)).toEqual(['only.txt']);
  });
});

describe('a whole untracked directory', () => {
  it('is one row with a name, not a row with none', () => {
    // git reports an untracked directory as a single entry with a trailing slash. Split as
    // written, the last segment is the empty string and the row shows nothing at all.
    const nodes = buildTree([{ path: 'tools/newdir/' }, { path: 'tools/other.txt' }]);
    const dir = nodes[0];
    if (dir?.kind !== 'dir') throw new Error('expected tools/');

    const names = dir.children.map((c) => c.name);
    expect(names).toContain('newdir/');
    expect(names).toContain('other.txt');
  });

  it('keeps the path git gave it, which is what staging acts on', () => {
    const nodes = buildTree([{ path: 'tools/newdir/' }]);
    expect(filesIn(nodes[0]!).map((f) => f.path)).toEqual(['tools/newdir/']);
  });

  it('ignores a path that is nothing but slashes', () => {
    expect(buildTree([{ path: '/' }, { path: '' }])).toEqual([]);
  });
});
