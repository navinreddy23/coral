<script lang="ts">
  import { hostingCreate, type PullRequest } from '../ipc/commands';
  import { messageOf } from '../ipc/error';

  const { repo, source, targets, label, onClose, onOpened }: {
    repo: string;
    /** The branch being proposed, which is the one the menu was opened on. */
    source: string;
    /** Branch names the host could merge into, without their remote prefix. */
    targets: string[];
    /** What the host calls one: GitHub says pull, GitLab says merge. */
    label: string;
    onClose: () => void;
    /** Handed the request the host created, so the window can offer to open it. */
    onOpened: (request: PullRequest) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let title = $state(source);
  let body = $state('');
  let draft = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  /**
   * Where it would land: the first of the usual names the host actually has.
   *
   * Guessed rather than asked for, because the answer is the same on almost every repository
   * and getting it wrong costs one click on a list that is right there.
   */
  // svelte-ignore state_referenced_locally
  let target = $state(
    ['main', 'master', 'develop', 'trunk'].find((name) => targets.includes(name)) ??
      targets[0] ??
      '',
  );

  const ready = $derived(title.trim().length > 0 && target !== '' && target !== source && !busy);

  async function create() {
    busy = true;
    error = null;
    try {
      const made = await hostingCreate(repo, title.trim(), body.trim(), source, target, draft);
      onOpened(made);
      onClose();
    } catch (e) {
      error = messageOf(e);
    } finally {
      busy = false;
    }
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
      event.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="panel">
  <header>
    <button class="back" onclick={onClose}>← Close</button>
    <p class="heading">New {label.toLowerCase()}</p>
  </header>

  <div class="form">
    <p class="where">
      <span class="branch">{source}</span>
      <span class="arrow" aria-hidden="true">→</span>
      {#if targets.length === 0}
        <span class="muted">no branch on the remote to merge into</span>
      {:else}
        <select bind:value={target} disabled={busy}>
          {#each targets as name (name)}
            <option value={name}>{name}</option>
          {/each}
        </select>
      {/if}
    </p>

    <label class="field">
      <span>Title</span>
      <input bind:value={title} disabled={busy} placeholder="What this changes" />
    </label>

    <label class="field">
      <span>Description</span>
      <textarea bind:value={body} rows="6" disabled={busy} placeholder="Why, and anything a reviewer needs"></textarea>
    </label>

    <label class="draft">
      <input type="checkbox" bind:checked={draft} disabled={busy} />
      Open it as a draft
    </label>

    {#if target === source && targets.length > 0}
      <p class="bad">A branch cannot be merged into itself.</p>
    {/if}
    {#if error !== null}
      <p class="bad">{error}</p>
    {/if}

    <div class="acts">
      <button class="go" disabled={!ready} onclick={() => void create()}>
        {busy ? 'Opening…' : `Open the ${label.toLowerCase()}`}
      </button>
    </div>
  </div>
</div>

<style>
  .panel {
    position: absolute; inset: 0; z-index: 12; display: flex; flex-direction: column;
    background: var(--bg-0);
  }
  header { padding: var(--space-2); border-bottom: 1px solid var(--border); background: var(--bg-1); }
  .back {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: var(--space-2);
    border-radius: var(--radius-1); background: var(--bg-1); border: 0; color: var(--accent);
  }
  .back:hover { background: var(--bg-2); }
  .heading {
    margin: var(--space-2) 0 0 var(--space-2); font-size: var(--text-lg); font-weight: 600;
    color: var(--fg-0);
  }
  .form { padding: var(--space-4); display: flex; flex-direction: column; gap: var(--space-3); max-width: 46em; }
  .where { margin: 0; display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-md); }
  .branch {
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-base);
    padding: 1px var(--space-2); border-radius: 999px;
    background: var(--accent-soft); color: var(--fg-0);
  }
  .arrow { color: var(--fg-2); }
  .field { display: flex; flex-direction: column; gap: 4px; font-size: var(--text-base); color: var(--fg-2); }
  input, textarea, select {
    font: inherit; font-size: var(--text-md); color: var(--fg-0);
    padding: var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-1); border: 1px solid var(--border-strong);
  }
  textarea { resize: vertical; font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-base); }
  .draft { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-md); color: var(--fg-1); }
  .bad { margin: 0; color: var(--danger); font-size: var(--text-base); }
  .acts { display: flex; gap: var(--space-2); }
  .go {
    font: inherit; font-size: var(--text-md); cursor: pointer;
    padding: var(--space-2) var(--space-4); border-radius: var(--radius-1);
    background: var(--accent); border: 1px solid var(--accent); color: var(--accent-fg);
  }
  .go:disabled { background: var(--bg-2); border-color: var(--border-strong); color: var(--fg-2); cursor: default; }
  .muted { color: var(--fg-2); font-size: var(--text-base); }
</style>
