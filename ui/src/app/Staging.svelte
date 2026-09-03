<script lang="ts">
  import type { StatusEntry } from '../ipc/types';
  import type { WorktreeState } from '../state/worktree.svelte';

  const { worktree }: { worktree: WorktreeState } = $props();

  let message = $state('');
  let amend = $state(false);

  const total = $derived(worktree.staged.length);
  const canCommit = $derived(
    message.trim().length > 0 && (total > 0 || amend) && !worktree.busy,
  );

  async function commit() {
    await worktree.commit(message, amend);
    if (!worktree.error) {
      message = '';
      amend = false;
    }
  }

  function label(e: StatusEntry): string {
    if (e.conflict) return e.conflict.replace(/_/g, ' ');
    const side = e.index !== 'unmodified' ? e.index : e.worktree;
    return side;
  }
</script>

<div class="panel">
  {#if worktree.error}
    <p class="error">{worktree.error}</p>
  {/if}

  {#if worktree.conflicted.length > 0}
    <h3 class="conflict">Conflicts ({worktree.conflicted.length})</h3>
    <ul>
      {#each worktree.conflicted as e (e.path)}
        <li><span class="mark conflict">!</span><span class="path">{e.path}</span></li>
      {/each}
    </ul>
  {/if}

  <h3>
    Staged ({worktree.staged.length})
    {#if worktree.staged.length > 0}
      <button onclick={() => worktree.stage(worktree.staged.map((e) => e.path), false)}>
        Unstage all
      </button>
    {/if}
  </h3>
  <ul>
    {#each worktree.staged as e (e.path)}
      <li>
        <span class="mark">{label(e).charAt(0).toUpperCase()}</span>
        <span class="path" title={e.path}>{e.path}</span>
        <button class="act" onclick={() => worktree.stage([e.path], false)}>−</button>
      </li>
    {/each}
  </ul>

  <h3>
    Unstaged ({worktree.unstaged.length})
    {#if worktree.unstaged.length > 0}
      <button onclick={() => worktree.stage(worktree.unstaged.map((e) => e.path), true)}>
        Stage all
      </button>
    {/if}
  </h3>
  <ul>
    {#each worktree.unstaged as e (e.path)}
      <li>
        <span class="mark">{label(e).charAt(0).toUpperCase()}</span>
        <span class="path" title={e.path}>{e.path}</span>
        <button class="act" onclick={() => worktree.stage([e.path], true)}>+</button>
      </li>
    {/each}
  </ul>

  <textarea
    class="message"
    rows="3"
    placeholder="Commit message"
    bind:value={message}
  ></textarea>
  <label class="amend">
    <input type="checkbox" bind:checked={amend} />
    Amend the previous commit
  </label>
  <button class="commit" disabled={!canCommit} onclick={commit}>
    {amend ? 'Amend' : `Commit ${total} file${total === 1 ? '' : 's'}`}
  </button>
</div>

<style>
  .panel { display: flex; flex-direction: column; gap: var(--space-2); font-size: 12px; }
  h3 {
    display: flex; align-items: center; gap: var(--space-2);
    font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em;
    color: var(--fg-2); margin: var(--space-2) 0 0;
  }
  h3.conflict { color: var(--danger); }
  h3 button {
    margin-left: auto; font: inherit; font-size: 11px; text-transform: none;
    background: none; border: 0; color: var(--accent); cursor: pointer;
  }
  ul { list-style: none; margin: 0; padding: 0; max-height: 30vh; overflow-y: auto; }
  li { display: flex; align-items: baseline; gap: var(--space-2); padding: 1px 0; }
  .mark { width: 1em; flex: 0 0 auto; font-family: var(--font-mono); color: var(--fg-2); }
  .mark.conflict { color: var(--danger); }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis;
          white-space: nowrap; direction: rtl; text-align: left; }
  .act {
    flex: 0 0 auto; font: inherit; width: 1.5em; cursor: pointer;
    background: var(--bg-2); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1);
  }
  .act:hover { background: var(--bg-3); }
  .message {
    font: inherit; font-size: 12px; resize: vertical;
    padding: var(--space-2); border: 1px solid var(--border); border-radius: 3px;
    background: var(--bg-0); color: var(--fg-0);
  }
  .amend { display: flex; align-items: center; gap: var(--space-2); color: var(--fg-1); }
  .commit {
    font: inherit; padding: var(--space-2); cursor: pointer;
    background: var(--accent); color: #fff; border: 0; border-radius: 3px;
  }
  .commit:disabled { background: var(--bg-3); color: var(--fg-2); cursor: default; }
  .error { color: var(--danger); margin: 0; }
</style>
