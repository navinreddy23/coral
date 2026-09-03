<script lang="ts">
  import FileTree from './FileTree.svelte';
  import { buildTree } from '../diff/tree';
  import type { CommitDetail } from '../ipc/types';

  const { detail, loading, error, openPath, onOpenFile }: {
    detail: CommitDetail | null;
    loading: boolean;
    error: string | null;
    /** Path whose diff is on screen, so the list can mark it. */
    openPath: string | null;
    onOpenFile: (path: string) => void;
  } = $props();

  function absolute(seconds: number): string {
    return new Date(seconds * 1000).toLocaleString();
  }

  /** A commit that was rebased or cherry-picked has two different answers. */
  const rewritten = $derived(
    detail !== null &&
      (detail.commit.author.name !== detail.commit.committer.name ||
        detail.commit.author.time !== detail.commit.committer.time),
  );

  type Grouping = 'path' | 'tree';
  let grouping = $state<Grouping>('path');
  let showAll = $state(false);

  /** How many files are listed before the rest are summarised. */
  const LIMIT = 500;

  const files = $derived(detail?.files ?? []);
  const shown = $derived(showAll ? files : files.slice(0, LIMIT));
  const tree = $derived(buildTree(shown));

  /** In tree mode the directory is a heading and only the file name repeats. */
  function split(path: string): { dir: string; name: string } {
    const at = path.lastIndexOf('/');
    return at < 0
      ? { dir: '', name: path }
      : { dir: path.slice(0, at + 1), name: path.slice(at + 1) };
  }

  const mark: Record<string, string> = {
    added: 'A',
    deleted: 'D',
    modified: 'M',
    renamed: 'R',
    copied: 'C',
  };
</script>

<aside>
  {#if error}
    <p class="error">{error}</p>
  {:else if loading}
    <p class="muted">Loading…</p>
  {:else if !detail}
    <p class="muted">Select a commit.</p>
  {:else}
    <h2>{detail.commit.summary}</h2>
    {#if detail.commit.body}
      <pre class="body">{detail.commit.body}</pre>
    {/if}

    <!--
      Exactly one dd per dt. The grid places items in order, so a second dd for the same term
      flows back into the label column and its content — an unbreakable object id — sets that
      column's width, leaving nothing for the values.
    -->
    <dl>
      <dt>Author</dt>
      <dd>
        {detail.commit.author.name} &lt;{detail.commit.author.email}&gt;
        <span class="when">{absolute(detail.commit.author.time)}</span>
      </dd>
      {#if rewritten}
        <dt>Committer</dt>
        <dd>
          {detail.commit.committer.name} &lt;{detail.commit.committer.email}&gt;
          <span class="when">{absolute(detail.commit.committer.time)}</span>
        </dd>
      {/if}
      <dt>Commit</dt>
      <dd class="mono break">{detail.commit.oid}</dd>
      {#if detail.commit.parents.length > 0}
        <dt>{detail.commit.parents.length > 1 ? 'Parents' : 'Parent'}</dt>
        <dd class="mono">
          {#each detail.commit.parents as parent (parent)}
            <span class="parent">{parent.slice(0, 12)}</span>
          {/each}
        </dd>
      {/if}
    </dl>

    <h3>{files.length} file{files.length === 1 ? '' : 's'}</h3>
    <div class="filebar">
      <div class="toggle">
        <button class:on={grouping === 'path'} onclick={() => (grouping = 'path')}>Path</button>
        <button class:on={grouping === 'tree'} onclick={() => (grouping = 'tree')}>Tree</button>
      </div>
      <label class="all">
        <input type="checkbox" bind:checked={showAll} />
        View all files
      </label>
    </div>
    {#if grouping === 'tree'}
      <FileTree nodes={tree} {openPath} {onOpenFile} />
    {:else}
    <ul class="files">
      {#each shown as file (file.path)}
        <li>
          <button
            class="file"
            class:open={file.path === openPath}
            onclick={() => onOpenFile(file.path)}
            title={file.oldPath ? `${file.path}\nfrom ${file.oldPath}` : file.path}
          >
            <span class="mark {file.change}">{mark[file.change] ?? '?'}</span>
            <span class="path">
              <span class="dir">{split(file.path).dir}</span
              ><span class="name">{split(file.path).name}</span>
            </span>
          </button>
        </li>
      {/each}
      {#if !showAll && files.length > LIMIT}
        <li class="muted">…and {files.length - LIMIT} more; tick “View all files” to list them</li>
      {/if}
    </ul>
    {/if}
  {/if}
</aside>

<style>
  aside {
    width: var(--details-w, 340px); flex: 0 0 auto; overflow-y: auto;
    border-left: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-3); font-size: 12px;
  }
  h2 {
    font-size: 14px; font-weight: 600; line-height: 1.35;
    margin: 0 0 var(--space-3); color: var(--fg-0);
  }
  /*
   * Section headings are set as a rule with a label on it, so the panel reads as a few short
   * sections rather than one long column of similar-looking lines.
   */
  h3 {
    display: flex; align-items: center; gap: var(--space-2);
    font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em;
    color: var(--fg-2); margin: var(--space-4) 0 var(--space-2);
  }
  h3::after {
    content: ''; flex: 1; height: 1px; background: var(--border);
  }
  .body {
    margin: 0 0 var(--space-3); padding: var(--space-2) var(--space-3);
    background: var(--bg-2); border-radius: var(--radius-1);
    /* A rule down the leading edge, as a quoted message is set everywhere else. */
    box-shadow: inset 2px 0 0 var(--border-strong);
    font-family: var(--font-mono); font-size: 11px; line-height: 1.5;
    white-space: pre-wrap; word-break: break-word; color: var(--fg-1);
  }
  dl {
    display: grid;
    /* Capped rather than max-content: the label column must never be able to squeeze the
       values out, however long a term the panel is asked to show. */
    grid-template-columns: fit-content(35%) minmax(0, 1fr);
    gap: var(--space-2) var(--space-3); margin: 0;
  }
  dt {
    color: var(--fg-2); font-size: 11px; text-transform: uppercase; letter-spacing: 0.05em;
    padding-top: 1px;
  }
  /* An address or an object id has no space to break at, so it would otherwise run past the
     panel edge and be clipped rather than wrapping. */
  dd { margin: 0; min-width: 0; color: var(--fg-1); overflow-wrap: anywhere; }
  dd.break { word-break: break-all; }
  .when { display: block; color: var(--fg-2); }
  .parent { display: block; }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  .filebar {
    display: flex; align-items: center; gap: var(--space-3);
    margin-bottom: var(--space-2);
  }
  .toggle {
    display: flex; border: 1px solid var(--border); border-radius: var(--radius-1);
    overflow: hidden;
  }
  .toggle button {
    font: inherit; font-size: 11px; cursor: pointer; padding: 1px var(--space-2);
    background: var(--bg-0); border: 0; color: var(--fg-2);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on { background: var(--accent); color: var(--accent-fg); font-weight: 600; }
  .all { display: flex; align-items: center; gap: var(--space-1); font-size: 11px; color: var(--fg-2); }
  /* The directory gives way first; a truncated path that has lost its file name identifies
     nothing, which is the one part that must always survive. */
  .dir { flex: 0 1 auto; min-width: 0; color: var(--fg-2);
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .name { flex: 0 0 auto; color: var(--fg-0); }
  .files { list-style: none; margin: 0; padding: 0; }
  .files li { display: flex; }
  /* Each file is the target for its own diff, so the whole row lights up rather than the
     path text alone. */
  .file {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; font: inherit; font-size: 12px;
    padding: 2px var(--space-2); margin: 0 calc(-1 * var(--space-2));
    background: none; border: 0; border-radius: var(--radius-1); cursor: pointer;
    color: var(--fg-1); overflow: hidden;
  }
  .file:hover { background: var(--bg-2); }
  .file.open {
    background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line);
  }
  .path { flex: 1; min-width: 0; display: flex; overflow: hidden; white-space: nowrap; }
  .from { color: var(--fg-2); font-size: 11px; }
  /*
   * The change letter on a tinted square. A bare coloured letter at 11px is a smudge; the
   * plate gives it an edge and makes the column scannable.
   */
  .mark {
    flex: 0 0 auto; width: 15px; height: 15px; line-height: 15px; text-align: center;
    font-family: var(--font-mono); font-size: 10px; font-weight: 700;
    border-radius: 3px; background: var(--bg-2); color: var(--fg-2);
  }
  .mark.added { color: var(--ok); background: var(--ok-soft); }
  .mark.deleted { color: var(--danger); background: var(--danger-soft); }
  .mark.modified { color: var(--lane-1); }
  .mark.renamed, .mark.copied { color: var(--lane-5); }
</style>
