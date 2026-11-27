<script module lang="ts">
  /** What the form asks git for. A null path leaves it to git; a null key takes the repository's. */
  export interface SubmoduleRequest {
    url: string;
    path: string | null;
    branch: string | null;
    key: string | null;
  }
</script>

<script lang="ts">
  import { handBackFocus } from './handback.svelte';
  import Close from './Close.svelte';
  import { baseName, submodulePath } from './path';
  import type { SshKey } from '../ipc/types';

  const {
    sshKeys,
    busy,
    error,
    onClose,
    onAdd,
  }: {
    /** Every key pair on this machine, for the picker to offer. */
    sshKeys: SshKey[];
    busy: boolean;
    error: string | null;
    onClose: () => void;
    onAdd: (request: SubmoduleRequest) => void;
  } = $props();

  let url = $state('');
  let typedPath = $state('');
  /** Once the path has been typed in, the URL stops filling it. */
  let pathTyped = $state(false);
  let branch = $state('');
  let key = $state('');

  const path = $derived(pathTyped ? typedPath : submodulePath(url));

  /**
   * A path on this machine, which git refuses to clone into a submodule unless told otherwise
   * (CVE-2022-39253). Said before the click, because git's own answer — "transport 'file' not
   * allowed" — names neither the setting nor why. A relative URL is not one of these: it is
   * resolved against this repository's remote, which is usually a server.
   */
  const local = $derived(/^(?:file:\/\/|\/|~|[A-Za-z]:[\\/]|\\\\)/u.test(url.trim()));

  const canAdd = $derived(!busy && url.trim() !== '');

  function add() {
    if (!canAdd) return;
    onAdd({
      url: url.trim(),
      path: path.trim() || null,
      branch: branch.trim() || null,
      key: key || null,
    });
  }

  /** Enter in a field does what the button does, as it does in every other form. */
  function addOnEnter(event: KeyboardEvent) {
    if (event.key !== 'Enter' || !(event.target instanceof HTMLInputElement)) return;
    event.preventDefault();
    add();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') onClose();
  }

  handBackFocus();
</script>

<svelte:window onkeydown={onKey} />

<div class="sheet" role="dialog" aria-modal="true" aria-label="Add a submodule">
  <header>
    <span class="glyph" aria-hidden="true">◱</span>
    <h2>Add a submodule</h2>
    <Close label="Close (Esc)" onclick={onClose} />
  </header>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="body" onkeydown={addOnEnter}>
    <label>
      Repository URL
      <input
        bind:value={url}
        placeholder="git@github.com:owner/repo.git"
        spellcheck="false"
        {@attach (node) => node.focus()}
      />
    </label>
    {#if local}
      <p class="note">
        git refuses a path on this machine as a submodule unless
        <code>protocol.file.allow</code> is set to <code>always</code>.
      </p>
    {/if}

    <label>
      Path
      <input
        value={path}
        placeholder="where it goes in this repository"
        spellcheck="false"
        oninput={(e) => {
          typedPath = e.currentTarget.value;
          pathTyped = true;
        }}
      />
    </label>

    <label>
      Branch
      <input bind:value={branch} placeholder="the remote's default" spellcheck="false" />
    </label>

    {#if sshKeys.length > 0}
      <label class="sshkey">
        SSH key
        <!-- A handler rather than `bind:value`, as the submodule panel does it: the keys arrive
             after the form opens, and a binding re-selects from state once they render. -->
        <select value={key} disabled={busy} onchange={(e) => (key = e.currentTarget.value)}>
          <option value="">The key this repository uses</option>
          {#each sshKeys as k (k.path)}
            <option value={k.path}>{baseName(k.path) || k.path} · {k.comment}</option>
          {/each}
        </select>
      </label>
    {/if}

    <p class="note">
      It is cloned, with any submodules of its own, and staged with <code>.gitmodules</code>.
      Committing records it.
    </p>

    {#if error}<p class="error">{error}</p>{/if}

    <button class="primary" disabled={!canAdd} onclick={add}>
      {busy ? 'Adding…' : 'Add submodule'}
    </button>
  </div>
</div>

<style>
  .sheet {
    position: absolute; inset: 0; z-index: 30;
    display: flex; flex-direction: column;
    background: var(--bg-1); color: var(--fg-0);
  }
  header {
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
  }
  .glyph { color: var(--fg-2); }
  h2 { margin: 0; font-size: var(--text-lg); font-weight: var(--weight-semibold); flex: 1; }

  .body {
    flex: 1; min-height: 0; overflow-y: auto;
    padding: var(--space-4); display: flex; flex-direction: column; gap: var(--space-3);
    align-items: flex-start; font-size: var(--text-base);
  }
  label {
    display: flex; flex-direction: column; gap: var(--space-1);
    font-size: var(--text-sm); color: var(--fg-2); width: 100%; max-width: 44em;
  }
  input, select {
    font: inherit; font-size: var(--text-base);
    padding: var(--space-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  input { font-family: var(--font-mono); font-variant-ligatures: none; }
  input:focus, select:focus { border-color: var(--accent); }
  select:disabled { opacity: 0.5; }
  .note {
    margin: -2px 0 0; max-width: 44em; font-size: var(--text-sm);
    color: var(--fg-2);
  }
  .note code { font-family: var(--font-mono); font-variant-ligatures: none; }

  button.primary {
    font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: var(--space-2) var(--space-3); border-radius: var(--radius-1);
    background: var(--accent); border: 1px solid var(--accent); color: var(--accent-fg);
    font-weight: var(--weight-semibold);
  }
  button.primary:hover:not(:disabled) { background: var(--accent-hover); }
  button:disabled { opacity: 0.5; cursor: default; }
  .error { margin: 0; color: var(--danger); max-width: 44em; white-space: pre-wrap; }
</style>
