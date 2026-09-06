<script lang="ts">
  import FileTree from './FileTree.svelte';
  import type { TreeNode } from '../diff/tree';
  import type { FileChange } from '../ipc/types';

  /**
   * What the tree needs of an entry.
   *
   * A path with no change of its own is a file the commit did not touch, which the "all
   * files" view is full of: it draws without a mark rather than not at all.
   */
  export interface Entry {
    path: string;
    oldPath: string | null;
    change: FileChange | null;
  }

  const { nodes, openPath, onOpenFile, depth = 0, startClosed = false }: {
    nodes: TreeNode<Entry>[];
    openPath: string | null;
    onOpenFile: (path: string) => void;
    depth?: number;
    /**
     * Whether directories start shut.
     *
     * They start open for a commit's own files, where the tree exists to show where the
     * changes landed and a list of closed folders shows nothing. The whole repository is a
     * different thing: the kernel has ninety-six thousand files, and drawing them all is a
     * panel that never paints.
     */
    startClosed?: boolean;
  } = $props();

  let closed = $state<Record<string, boolean>>({});
  const shut = (path: string) => closed[path] ?? startClosed;

  const mark: Record<string, string> = {
    added: 'A',
    deleted: 'D',
    modified: 'M',
    renamed: 'R',
    copied: 'C',
    unmerged: '!',
  };
</script>

<ul class="tree">
  {#each nodes as node (`${node.kind}:${node.path}`)}
    <li>
      {#if node.kind === 'dir'}
        <button
          class="dir"
          style:padding-left="{depth * 12 + 4}px"
          onclick={() => (closed[node.path] = !shut(node.path))}
        >
          <span class="caret">{shut(node.path) ? '›' : '⌄'}</span>{node.name}
        </button>
        {#if !shut(node.path)}
          <FileTree
            nodes={node.children}
            {openPath}
            {onOpenFile}
            {startClosed}
            depth={depth + 1}
          />
        {/if}
      {:else}
        <button
          class="file"
          class:open={node.path === openPath}
          style:padding-left="{depth * 12 + 4}px"
          onclick={() => onOpenFile(node.path)}
          title={node.item.oldPath ? `${node.path}\nfrom ${node.item.oldPath}` : node.path}
        >
          <span class="mark {node.item.change ?? 'untouched'}">
            {node.item.change === null ? '' : mark[node.item.change] ?? '?'}
          </span>
          {node.name}
        </button>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .tree { list-style: none; margin: 0; padding: 0; }
  button {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; cursor: pointer;
    font: inherit; font-size: 12px; padding: 1px var(--space-2) 1px 4px;
    background: var(--bg-1); border: 0; color: var(--fg-1);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  button:hover { background: var(--bg-2); }
  .file.open { background: var(--accent-soft); color: var(--fg-0); }
  .dir { color: var(--fg-2); }
  .caret { width: 9px; flex: 0 0 auto; }
  .mark { flex: 0 0 auto; width: 10px; font-family: var(--font-mono); font-size: 10px; }
  .mark.added { color: var(--ok); }
  .mark.deleted { color: var(--danger); }
  .mark.renamed, .mark.copied { color: var(--accent); }
  .mark.modified { color: var(--fg-2); }
</style>
