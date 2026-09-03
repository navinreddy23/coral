<script lang="ts">
  import type { CommitDetail } from '../ipc/types';

  const { detail, loading, error }: {
    detail: CommitDetail | null;
    loading: boolean;
    error: string | null;
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

    <dl>
      <dt>Author</dt>
      <dd>{detail.commit.author.name} &lt;{detail.commit.author.email}&gt;</dd>
      <dd class="muted">{absolute(detail.commit.author.time)}</dd>
      {#if rewritten}
        <dt>Committer</dt>
        <dd>{detail.commit.committer.name} &lt;{detail.commit.committer.email}&gt;</dd>
        <dd class="muted">{absolute(detail.commit.committer.time)}</dd>
      {/if}
      <dt>Commit</dt>
      <dd class="mono break">{detail.commit.oid}</dd>
      {#if detail.commit.parents.length > 0}
        <dt>{detail.commit.parents.length > 1 ? 'Parents' : 'Parent'}</dt>
        {#each detail.commit.parents as parent (parent)}
          <dd class="mono">{parent.slice(0, 12)}</dd>
        {/each}
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
    <ul class="files">
      {#each shown as file (file.path)}
        <li>
          <span class="mark {file.change}">{mark[file.change] ?? '?'}</span>
          {#if grouping === 'path'}
            <span class="path" title={file.path}>
              <span class="dir">{split(file.path).dir}</span>{split(file.path).name}
            </span>
          {:else}
            <span class="path" title={file.path}>{split(file.path).name}</span>
            <span class="from">{split(file.path).dir || './'}</span>
          {/if}
          {#if file.oldPath}<span class="from">from {file.oldPath}</span>{/if}
        </li>
      {/each}
      {#if !showAll && files.length > LIMIT}
        <li class="muted">…and {files.length - LIMIT} more; tick “View all files” to list them</li>
      {/if}
    </ul>
  {/if}
</aside>

<style>
  aside {
    width: 340px; flex: 0 0 auto; overflow-y: auto;
    border-left: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-3); font-size: 12px;
  }
  h2 { font-size: 13px; font-weight: 600; margin: 0 0 var(--space-2); color: var(--fg-0); }
  h3 { font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em;
       color: var(--fg-2); margin: var(--space-4) 0 var(--space-2); }
  .body {
    margin: 0 0 var(--space-3); padding: var(--space-2);
    background: var(--bg-2); border-radius: 3px;
    font-family: var(--font-mono); font-size: 11px;
    white-space: pre-wrap; word-break: break-word; color: var(--fg-1);
  }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px var(--space-3); margin: 0; }
  dt { color: var(--fg-2); }
  dd { margin: 0; color: var(--fg-1); }
  dd.break { word-break: break-all; }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  .filebar {
    display: flex; align-items: center; gap: var(--space-3);
    margin-bottom: var(--space-2);
  }
  .toggle { display: flex; border: 1px solid var(--border); border-radius: 3px; overflow: hidden; }
  .toggle button {
    font: inherit; font-size: 11px; cursor: pointer; padding: 1px var(--space-2);
    background: var(--bg-0); border: 0; color: var(--fg-1);
  }
  .toggle button.on { background: var(--bg-3); color: var(--fg-0); }
  .all { display: flex; align-items: center; gap: var(--space-1); font-size: 11px; color: var(--fg-2); }
  .dir { color: var(--fg-2); }
  .files { list-style: none; margin: 0; padding: 0; }
  .files li { display: flex; gap: var(--space-2); align-items: baseline; padding: 1px 0; }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .from { color: var(--fg-2); font-size: 11px; }
  .mark { width: 1em; flex: 0 0 auto; font-family: var(--font-mono); }
  .mark.added { color: var(--lane-4); }
  .mark.deleted { color: var(--danger); }
  .mark.modified { color: var(--lane-1); }
  .mark.renamed, .mark.copied { color: var(--lane-5); }
</style>
