/**
 * A list of paths as a directory tree.
 *
 * Paths come from git as slash-separated bytes regardless of platform, so this splits on '/'
 * and never on the host separator.
 *
 * Generic over what hangs off a leaf, because both the files a commit changed and the files
 * waiting in the working tree are lists of paths and want the same rows.
 */
export interface TreeDir<T> {
  kind: 'dir';
  /** What the row shows: several segments when a chain of single-child directories collapsed. */
  name: string;
  /** Full path from the repository root, which is what identifies the row. */
  path: string;
  children: TreeNode<T>[];
}

export interface TreeFile<T> {
  kind: 'file';
  name: string;
  path: string;
  item: T;
}

export type TreeNode<T> = TreeDir<T> | TreeFile<T>;

/** Anything with a repository-relative path can be arranged into one of these. */
export interface Pathed {
  path: string;
}

/**
 * Groups items into a tree, collapsing runs of directories that hold nothing else.
 *
 * A kernel commit touching `drivers/net/ethernet/intel/ice/ice_main.c` would otherwise cost
 * five rows and five levels of indent to show one file, which is what makes an uncollapsed
 * tree unusable on a deep repository.
 */
export function buildTree<T extends Pathed>(items: readonly T[]): TreeNode<T>[] {
  const root: TreeDir<T> = { kind: 'dir', name: '', path: '', children: [] };

  for (const item of items) {
    // A trailing slash is git reporting a whole untracked directory as one entry. Splitting it
    // as written leaves a last segment that is the empty string, and the row shows no name at
    // all; the slash is kept on the label so it still reads as a directory.
    const directory = item.path.endsWith('/');
    const segments = item.path.replace(/\/+$/u, '').split('/');
    const last = segments.pop();
    if (last === undefined || last === '') continue;

    let at = root;
    for (const segment of segments) {
      at = childDir(at, segment);
    }
    at.children.push({
      kind: 'file',
      name: directory ? `${last}/` : last,
      path: item.path,
      item,
    });
  }

  sort(root);
  return collapse(root).children;
}

/**
 * Every item under a node, itself included when it is a file.
 *
 * What a directory row needs to say how many changes are inside it, and what staging a whole
 * directory acts on.
 */
export function filesIn<T>(node: TreeNode<T>): T[] {
  if (node.kind === 'file') return [node.item];
  return node.children.flatMap(filesIn);
}

function childDir<T>(parent: TreeDir<T>, name: string): TreeDir<T> {
  for (const child of parent.children) {
    if (child.kind === 'dir' && child.name === name) return child;
  }
  const made: TreeDir<T> = {
    kind: 'dir',
    name,
    path: parent.path === '' ? name : `${parent.path}/${name}`,
    children: [],
  };
  parent.children.push(made);
  return made;
}

/** Directories before files, then by name, which is how a file manager orders them. */
function sort<T>(dir: TreeDir<T>): void {
  dir.children.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === 'dir' ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  for (const child of dir.children) {
    if (child.kind === 'dir') sort(child);
  }
}

function collapse<T>(dir: TreeDir<T>): TreeDir<T> {
  dir.children = dir.children.map((child) => (child.kind === 'dir' ? collapse(child) : child));

  const only = dir.children.length === 1 ? dir.children[0] : undefined;
  if (dir.path !== '' && only !== undefined && only.kind === 'dir') {
    return { ...only, name: `${dir.name}/${only.name}`, path: only.path };
  }
  return dir;
}
