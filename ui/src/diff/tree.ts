import type { ChangedFile } from '../ipc/types';

/**
 * A commit's changed files as a directory tree.
 *
 * Paths come from git as slash-separated bytes regardless of platform, so this splits on '/'
 * and never on the host separator.
 */
export interface TreeDir {
  kind: 'dir';
  /** What the row shows: several segments when a chain of single-child directories collapsed. */
  name: string;
  /** Full path from the repository root, which is what identifies the row. */
  path: string;
  children: TreeNode[];
}

export interface TreeFile {
  kind: 'file';
  name: string;
  file: ChangedFile;
}

export type TreeNode = TreeDir | TreeFile;

/**
 * Groups files into a tree, collapsing runs of directories that hold nothing else.
 *
 * A kernel commit touching `drivers/net/ethernet/intel/ice/ice_main.c` would otherwise cost
 * five rows and five levels of indent to show one file, which is what makes an uncollapsed
 * tree unusable on a deep repository.
 */
export function buildTree(files: readonly ChangedFile[]): TreeNode[] {
  const root: TreeDir = { kind: 'dir', name: '', path: '', children: [] };

  for (const file of files) {
    const segments = file.path.split('/');
    const name = segments.pop();
    if (name === undefined) continue;

    let at = root;
    for (const segment of segments) {
      at = childDir(at, segment);
    }
    at.children.push({ kind: 'file', name, file });
  }

  sort(root);
  return collapse(root).children;
}

function childDir(parent: TreeDir, name: string): TreeDir {
  for (const child of parent.children) {
    if (child.kind === 'dir' && child.name === name) return child;
  }
  const made: TreeDir = {
    kind: 'dir',
    name,
    path: parent.path === '' ? name : `${parent.path}/${name}`,
    children: [],
  };
  parent.children.push(made);
  return made;
}

/** Directories before files, then by name, which is how a file manager orders them. */
function sort(dir: TreeDir): void {
  dir.children.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === 'dir' ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  for (const child of dir.children) {
    if (child.kind === 'dir') sort(child);
  }
}

function collapse(dir: TreeDir): TreeDir {
  dir.children = dir.children.map((child) => (child.kind === 'dir' ? collapse(child) : child));

  const only = dir.children.length === 1 ? dir.children[0] : undefined;
  if (dir.path !== '' && only !== undefined && only.kind === 'dir') {
    return { ...only, name: `${dir.name}/${only.name}`, path: only.path };
  }
  return dir;
}
