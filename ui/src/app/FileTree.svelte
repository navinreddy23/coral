<script lang="ts">
  import Icon from './Icon.svelte';
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
          <span class="caret">
            <Icon name={shut(node.path) ? 'chevronRight' : 'chevronDown'} size={13} />
          </span><span class="name">{node.name}</span>
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
          <span class="name">{node.name}</span>
        </button>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .tree { list-style: none; margin: 0; padding: 0; }
  /*
   * The indent is padding, so the row has to count it inside its own width. Left outside, every
   * row was wider than the panel by its own indent — which gave the panel a sideways scrollbar
   * whatever was in the tree, and one stray scroll then carried the message, the object ids and
   * the controls off the left edge with it.
   */
  button {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; box-sizing: border-box; text-align: left; cursor: pointer;
    font: inherit; font-size: var(--text-base); padding: 1px var(--space-2) 1px 4px;
    background: var(--bg-1); border: 0; color: var(--fg-1);
    overflow: hidden; white-space: nowrap;
  }
  /* The name is what gives, since the mark and the caret are what the row is read by. Ellipsis
     on the button itself does nothing: a flex item does not shrink, so it was cut mid-letter. */
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  button:hover { background: var(--bg-2); }
  .file.open { background: var(--accent-soft); color: var(--fg-0); }
  .dir { color: var(--fg-2); }
  .caret { width: 9px; flex: 0 0 auto; }
  .mark { flex: 0 0 auto; width: 10px; font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-xs); }
  .mark.added { color: var(--ok); }
  .mark.deleted { color: var(--danger); }
  .mark.renamed, .mark.copied { color: var(--accent); }
  .mark.modified { color: var(--fg-2); }
</style>
