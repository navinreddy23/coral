<script lang="ts">
  import type { RemotesState } from '../state/remotes.svelte';

  const { remotes, focus, onClose, onChanged }: {
    remotes: RemotesState;
    /** Which remote to open on, when the panel was reached from one in the sidebar. */
    focus: string | null;
    onClose: () => void;
    /** Called after any edit, so the sidebar and graph can be reloaded. */
    onChanged: () => void;
  } = $props();

  /** The row being edited, by name, or `null` for the add form. */
  let editing = $state<string | null>(null);
  let adding = $state(false);

  let name = $state('');
  let url = $state('');
  let pushUrl = $state('');

  /**
   * Opens on the remote the panel was reached from, or on the add form when there are none.
   *
   * Once, not on every change: after the first edit the user is driving, and reasserting the
   * opening choice would drag them back to it.
   */
  let opened = $state(false);
  $effect(() => {
    if (opened) return;
    opened = true;
    if (focus !== null) startEdit(focus);
    else if (remotes.list.length === 0) adding = true;
  });

  /** Loads a remote's current values into the form. */
  function startEdit(remote: string) {
    const found = remotes.list.find((r) => r.name === remote);
    adding = false;
    editing = remote;
    name = found?.name ?? '';
    url = found?.fetchUrl ?? '';
    pushUrl = found?.pushUrl ?? '';
  }

  function startAdd() {
    adding = true;
    editing = null;
    name = '';
    url = '';
    pushUrl = '';
  }

  const chosen = $derived(remotes.list.find((r) => r.name === editing) ?? null);

  /**
   * Applies the form.
   *
   * A rename and a URL change are two git commands, and the rename goes first: `set-url` names
   * the remote, so doing it the other way round would address one that no longer exists.
   */
  async function save() {
    if (adding) {
      if (name.trim() === '' || url.trim() === '') return;
      if (await remotes.edit({ kind: 'add', name: name.trim(), url: url.trim() })) {
        adding = false;
        editing = name.trim();
        onChanged();
      }
      return;
    }
    if (!chosen) return;

    let current = chosen.name;
    if (name.trim() !== '' && name.trim() !== current) {
      if (!(await remotes.edit({ kind: 'rename', name: current, to: name.trim() }))) return;
      current = name.trim();
      editing = current;
    }
    if (url.trim() !== '' && url.trim() !== chosen.fetchUrl) {
      if (!(await remotes.edit({ kind: 'setUrl', name: current, url: url.trim() }))) return;
    }
    onChanged();
  }

  let confirming = $state<string | null>(null);

  async function remove(remote: string) {
    confirming = null;
    if (await remotes.edit({ kind: 'remove', name: remote })) {
      if (editing === remote) editing = remotes.list[0]?.name ?? null;
      onChanged();
    }
  }

  async function prune(remote: string) {
    if (await remotes.edit({ kind: 'prune', name: remote })) onChanged();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="sheet">
  <header>
    <h2>Remotes</h2>
    <button class="shut" onclick={onClose} title="Close">×</button>
  </header>

  <div class="split">
    <ul class="list">
      {#each remotes.list as remote (remote.name)}
        <li>
          <button
            class="pick"
            class:on={editing === remote.name && !adding}
            onclick={() => startEdit(remote.name)}
          >
            <span class="name">{remote.name}</span>
            <span class="url">{remote.fetchUrl}</span>
          </button>
        </li>
      {/each}
      <li>
        <button class="pick add" class:on={adding} onclick={startAdd}>+ Add a remote</button>
      </li>
    </ul>

    <div class="form">
      {#if adding}
        <h3>New remote</h3>
        <label>
          Name
          <input bind:value={name} placeholder="upstream" spellcheck="false" />
        </label>
        <label>
          URL
          <input bind:value={url} placeholder="git@github.com:owner/repo.git" spellcheck="false" />
        </label>
        <p class="hint">
          Tracking branches appear after the first fetch, not when the remote is added.
        </p>
      {:else if chosen}
        <h3>{chosen.name}</h3>
        <label>
          Name
          <input bind:value={name} spellcheck="false" />
        </label>
        <label>
          Fetch URL
          <input bind:value={url} spellcheck="false" />
        </label>
        {#if chosen.pushUrl !== chosen.fetchUrl}
          <label>
            Push URL
            <input value={pushUrl} readonly spellcheck="false" />
          </label>
          <p class="hint">
            This remote pushes somewhere else. Coral does not change a separate push URL.
          </p>
        {/if}
      {:else}
        <p class="hint">No remotes are configured. Add one to fetch and push.</p>
      {/if}

      {#if remotes.error}<p class="error">{remotes.error}</p>{/if}

      <div class="row">
        <button class="primary" disabled={remotes.busy || (adding && name.trim() === '')} onclick={save}>
          {adding ? 'Add remote' : 'Save changes'}
        </button>
        {#if chosen && !adding}
          <button disabled={remotes.busy} onclick={() => prune(chosen.name)}>
            Prune gone branches
          </button>
          {#if confirming === chosen.name}
            <button class="danger" onclick={() => remove(chosen.name)}>
              Really remove {chosen.name}
            </button>
            <button onclick={() => (confirming = null)}>Cancel</button>
          {:else}
            <button class="danger" onclick={() => (confirming = chosen.name)}>Remove</button>
          {/if}
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  /* Covers the panes rather than floating over them: this is a settings screen, and a dialog
     small enough to float would have to scroll to show a URL. */
  .sheet {
    position: absolute; inset: 0; z-index: 30;
    display: flex; flex-direction: column;
    background: var(--bg-1); color: var(--fg-0);
  }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
  }
  h2 { margin: 0; font-size: 14px; font-weight: 600; flex: 1; }
  .shut {
    font: inherit; font-size: 18px; line-height: 1; cursor: pointer;
    background: none; border: 0; color: var(--fg-2); padding: 0 var(--space-2);
  }
  .shut:hover { color: var(--fg-0); }

  .split { display: flex; flex: 1; min-height: 0; }
  .list {
    list-style: none; margin: 0; padding: var(--space-2);
    width: 17em; flex: 0 0 auto; overflow-y: auto;
    border-right: 1px solid var(--border);
  }
  .pick {
    display: flex; flex-direction: column; gap: 1px; align-items: flex-start;
    width: 100%; text-align: left; font: inherit; cursor: pointer;
    padding: var(--space-2); border-radius: var(--radius-1);
    background: none; border: 0; color: var(--fg-1); overflow: hidden;
  }
  .pick:hover { background: var(--bg-2); }
  .pick.on { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line); }
  .name { font-size: 12px; font-weight: 600; color: var(--fg-0); }
  .url {
    font-size: 11px; color: var(--fg-2); max-width: 100%;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .add { color: var(--accent); font-size: 12px; }

  .form {
    flex: 1; min-width: 0; overflow-y: auto;
    padding: var(--space-4); display: flex; flex-direction: column; gap: var(--space-3);
    align-items: flex-start;
  }
  h3 { margin: 0; font-size: 13px; font-weight: 600; }
  label {
    display: flex; flex-direction: column; gap: var(--space-1);
    font-size: 11px; color: var(--fg-2); width: 100%; max-width: 44em;
  }
  input {
    font: inherit; font-size: 12px; font-family: var(--font-mono);
    padding: var(--space-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  input:focus { border-color: var(--accent); }
  input[readonly] { color: var(--fg-2); background: var(--bg-2); }
  .hint { margin: 0; font-size: 11px; color: var(--fg-2); max-width: 40em; line-height: 1.5; }
  .error { margin: 0; color: var(--danger); font-size: 12px; max-width: 44em; }

  .row { display: flex; gap: var(--space-2); flex-wrap: wrap; }
  .row button {
    font: inherit; font-size: 12px; cursor: pointer;
    padding: var(--space-2) var(--space-3); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .row button:hover:not(:disabled) { background: var(--bg-3); color: var(--fg-0); }
  .row button:disabled { opacity: 0.5; cursor: default; }
  .row .primary {
    background: var(--accent); border-color: var(--accent); color: var(--accent-fg);
    font-weight: 600;
  }
  .row .primary:hover:not(:disabled) { background: var(--accent-hover); color: var(--accent-fg); }
  .row .danger { color: var(--danger); }
  .row .danger:hover:not(:disabled) { background: var(--danger-soft); color: var(--danger); }
</style>
