<script lang="ts" module>
  import type { StatusEntry } from '../ipc/types';

  /**
   * How a working-tree entry reads on a row: one letter and one colour.
   *
   * git's own letters, which are also the ones the commit panel and the file tree already
   * use. This list read `+ − ✎ R C T` — two arithmetic signs, a pencil and three letters —
   * so the same file changed the same way was marked differently depending on which panel
   * was showing it.
   */
  export function markOf(entry: StatusEntry, staged: boolean): string {
    if (entry.conflict) return 'U';
    const change = staged ? entry.index : entry.worktree;
    switch (change) {
      case 'added':
      case 'untracked':
        return 'A';
      case 'deleted':
        return 'D';
      case 'renamed':
        return 'R';
      case 'copied':
        return 'C';
      case 'type_changed':
        return 'T';
      default:
        return 'M';
    }
  }

  export function kindOf(entry: StatusEntry, staged: boolean): string {
    if (entry.conflict) return 'conflict';
    const change = staged ? entry.index : entry.worktree;
    return change === 'untracked' ? 'added' : change;
  }
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import Changes from './Changes.svelte';
  import { filesIn, type TreeNode } from '../diff/tree';

  const { nodes, staged, openPath, closed, onToggleDir, onAct, onOpen, onMenu, depth = 0 }: {
    nodes: TreeNode<StatusEntry>[];
    /** Which side this list is, which decides the verb and the letter shown. */
    staged: boolean;
    openPath: string | null;
    /** Collapsed directories, shared with the panel so "Expand all" can reach them. */
    closed: Record<string, boolean>;
    onToggleDir: (path: string) => void;
    /** Stage or unstage these paths — a file, or everything under a directory. */
    onAct: (paths: string[]) => void;
    onOpen: (path: string) => void;
    /** What can be done with one file, asked for by right-clicking it. */
    onMenu: (event: MouseEvent, path: string) => void;
    depth?: number;
  } = $props();

  /**
   * How many changes of each kind are under a directory.
   *
   * Shown on the folder row, as the reference does: a closed directory that only says its name
   * hides exactly the thing the panel exists to show.
   */
  function tally(node: TreeNode<StatusEntry>): { edits: number; adds: number } {
    let edits = 0;
    let adds = 0;
    for (const entry of filesIn(node)) {
      if (markOf(entry, staged) === 'A') adds += 1;
      else edits += 1;
    }
    return { edits, adds };
  }
</script>

<ul class="tree">
  {#each nodes as node (`${node.kind}:${node.path}`)}
    <li>
      {#if node.kind === 'dir'}
        {@const counts = tally(node)}
        <div class="row">
          <button
            class="dir"
            style:padding-left="{depth * 12 + 4}px"
            onclick={() => onToggleDir(node.path)}
          >
            <span class="caret">
              <Icon name={closed[node.path] ? 'chevronRight' : 'chevronDown'} size={13} />
            </span>
            <span class="name">{node.name}</span>
            {#if counts.edits > 0}<span class="count edit">M {counts.edits}</span>{/if}
            {#if counts.adds > 0}<span class="count add">A {counts.adds}</span>{/if}
          </button>
          <button
            class="act"
            title={staged ? `Unstage everything in ${node.name}` : `Stage everything in ${node.name}`}
            onclick={() => onAct(filesIn(node).map((e) => e.path))}
          >{staged ? '−' : '+'}</button>
        </div>
        {#if !closed[node.path]}
          <Changes
            nodes={node.children}
            {staged}
            {openPath}
            {closed}
            {onToggleDir}
            {onAct}
            {onOpen}
            {onMenu}
            depth={depth + 1}
          />
        {/if}
      {:else}
        <div class="row" role="presentation" oncontextmenu={(event) => onMenu(event, node.path)}>
          <button
            class="file"
            class:open={node.path === openPath}
            style:padding-left="{depth * 12 + 4}px"
            onclick={() => onOpen(node.path)}
            title={node.item.origPath ? `${node.path}\nfrom ${node.item.origPath}` : node.path}
          >
            <span class="mark {kindOf(node.item, staged)}">{markOf(node.item, staged)}</span>
            <span class="name">{node.name}</span>
          </button>
          <button
            class="act"
            title={staged ? 'Unstage it' : 'Stage it'}
            onclick={() => onAct([node.path])}
          >{staged ? '−' : '+'}</button>
        </div>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .tree { list-style: none; margin: 0; padding: 0; }
  /* The row is the file and its one action; the action appears on hover so a long list is not
     a column of buttons. */
  .row { display: flex; align-items: center; }
  .row:hover { background: var(--bg-2); }
  .dir, .file {
    display: flex; align-items: center; gap: var(--space-2);
    flex: 1; min-width: 0; text-align: left; cursor: pointer;
    font: inherit; font-size: var(--text-base); padding: 2px var(--space-2) 2px 4px;
    background: var(--bg-1); border: 0; color: var(--fg-1);
  }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dir { color: var(--fg-1); font-weight: 600; }
  .file.open { background: var(--accent-soft); color: var(--fg-0); }
  .caret { width: 9px; flex: 0 0 auto; color: var(--fg-2); }
  .mark {
    flex: 0 0 auto; width: 12px; text-align: center;
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); font-weight: 700;
  }
  .mark.added { color: var(--ok); }
  .mark.deleted { color: var(--danger); }
  .mark.renamed, .mark.copied, .mark.type_changed { color: var(--lane-5); }
  .mark.modified { color: var(--lane-1); }
  .mark.conflict { color: var(--danger); }
  /* Per-directory counts, so a folder says what is inside it without being opened. */
  .count {
    flex: 0 0 auto; margin-left: auto; font-size: var(--text-xs); font-variant-numeric: tabular-nums;
  }
  .count + .count { margin-left: var(--space-2); }
  .count.edit { color: var(--lane-1); }
  .count.add { color: var(--ok); }
  .act {
    flex: 0 0 auto; font: inherit; font-size: var(--text-md); line-height: 1; width: 1.6em;
    cursor: pointer; visibility: hidden;
    background: var(--bg-2); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1); margin-right: var(--space-1);
  }
  .row:hover .act { visibility: visible; }
  .act:hover { background: var(--bg-3); color: var(--fg-0); }
</style>
