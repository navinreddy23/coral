<script lang="ts">
  import Icon from './Icon.svelte';
  import type { Submodule, SubmoduleRevision } from '../ipc/types';

  const { submodule, revision, busy, error, onClose, onSetUrl, onOpen, onUpdate, onRemove }: {
    submodule: Submodule;
    /** Null while it loads, and for a submodule with no working copy to read one from. */
    revision: SubmoduleRevision | null;
    busy: boolean;
    error: string | null;
    onClose: () => void;
    onSetUrl: (url: string) => void;
    onOpen: () => void;
    /** `remote` moves it to its branch tip rather than to the recorded commit. */
    onUpdate: (remote: boolean) => void;
    onRemove: () => void;
  } = $props();

  let url = $state('');
  let seeded = $state('');

  // Reseeds when a different submodule is opened, not on every change, or typing is undone.
  $effect(() => {
    if (seeded === submodule.path) return;
    seeded = submodule.path;
    url = submodule.url;
  });

  const changed = $derived(url.trim() !== '' && url.trim() !== submodule.url);

  // git reports author time as a 64-bit count of seconds, which ts-rs types as bigint.
  function when(seconds: bigint | number): string {
    return new Date(Number(seconds) * 1000).toLocaleString();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="sheet">
  <header>
    <span class="glyph" aria-hidden="true">◱</span>
    <h2>Edit submodule</h2>
    <button class="shut" onclick={onClose} title="Close"><Icon name="close" size={15} /></button>
  </header>

  <div class="body">
    <label>
      Remote URL
      <input bind:value={url} spellcheck="false" />
    </label>

    <!--
      The path is shown but not editable. Moving a submodule is a `git mv` plus a rewrite of
      `.gitmodules` plus a rewrite of the clone's `core.worktree`, and a field that looked
      editable but silently did none of that would be worse than one that does not.
    -->
    <label>
      Path
      <input value={submodule.path} readonly spellcheck="false" />
    </label>

    <button class="primary" disabled={!changed || busy} onclick={() => onSetUrl(url.trim())}>
      Edit this submodule
    </button>

    {#if error}<p class="error">{error}</p>{/if}

    {#if !submodule.initialised}
      <p class="state absent">
        This submodule has no working copy yet. Update it to clone one.
      </p>
    {:else if revision === null}
      <p class="state">Reading the current revision…</p>
    {:else if revision.inSync}
      <p class="state ok">
        <Icon name="check" size={12} /> This submodule is at the commit this repository records.
      </p>
    {:else}
      <p class="state warn">
        This submodule is not at the commit the repository records. Updating moves it back.
      </p>
    {/if}

    {#if revision}
      <section class="revision">
        <h3>Current revision <span class="when">{when(revision.time)}</span></h3>
        <dl>
          <dt>Commit</dt>
          <dd class="mono">{revision.oid}</dd>
          <dt>Message</dt>
          <dd>{revision.summary}</dd>
        </dl>
      </section>
    {/if}

    <div class="row">
      <button disabled={!submodule.initialised} onclick={onOpen}>Open this submodule</button>
      <button disabled={busy} onclick={() => onUpdate(false)}>
        {submodule.initialised ? 'Update to the recorded commit' : 'Fetch a working copy'}
      </button>
      <button disabled={busy} onclick={() => onUpdate(true)}>Update to the branch tip</button>
      <button class="danger" disabled={busy} onclick={onRemove}>Delete this submodule</button>
    </div>
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
  h2 { margin: 0; font-size: var(--text-lg); font-weight: 600; flex: 1; }
  .shut {
    display: flex; font: inherit; line-height: 1; cursor: pointer;
    background: var(--bg-1); border: 0; color: var(--fg-2); padding: 0 var(--space-2);
  }
  .shut:hover { color: var(--fg-0); }

  .body {
    flex: 1; min-height: 0; overflow-y: auto;
    padding: var(--space-4); display: flex; flex-direction: column; gap: var(--space-3);
    align-items: flex-start; font-size: var(--text-base);
  }
  label {
    display: flex; flex-direction: column; gap: var(--space-1);
    font-size: var(--text-sm); color: var(--fg-2); width: 100%; max-width: 44em;
  }
  input {
    font: inherit; font-size: var(--text-base); font-family: var(--font-mono); font-variant-ligatures: none;
    padding: var(--space-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  input:focus { border-color: var(--accent); }
  input[readonly] { background: var(--bg-2); color: var(--fg-2); }

  .state {
    margin: 0; padding: var(--space-2) var(--space-3); border-radius: var(--radius-1);
    width: 100%; max-width: 44em; box-sizing: border-box;
  }
  .state.ok { background: var(--ok-soft); color: var(--ok); }
  .state.warn { background: var(--warn-soft); color: var(--warn); }
  .state.absent { background: var(--bg-2); color: var(--fg-1); }

  .revision { width: 100%; max-width: 44em; }
  h3 {
    display: flex; align-items: baseline; gap: var(--space-2);
    margin: 0 0 var(--space-2); font-size: var(--text-sm); font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.06em; color: var(--fg-2);
  }
  .when { text-transform: none; letter-spacing: 0; font-weight: 400; }
  dl {
    display: grid; grid-template-columns: fit-content(30%) minmax(0, 1fr);
    gap: var(--space-2) var(--space-3); margin: 0;
    padding: var(--space-3); background: var(--bg-2); border-radius: var(--radius-1);
  }
  dt { color: var(--fg-2); font-size: var(--text-sm); }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; color: var(--fg-1); }

  .row { display: flex; gap: var(--space-2); flex-wrap: wrap; }
  button:not(.shut) {
    font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: var(--space-2) var(--space-3); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  button:not(.shut):hover:not(:disabled) { background: var(--bg-3); color: var(--fg-0); }
  button:disabled { opacity: 0.5; cursor: default; }
  .primary {
    background: var(--accent); border-color: var(--accent); color: var(--accent-fg);
    font-weight: 600;
  }
  .primary:hover:not(:disabled) { background: var(--accent-hover); color: var(--accent-fg); }
  .danger { color: var(--danger); }
  .danger:hover:not(:disabled) { background: var(--danger-soft); color: var(--danger); }
  .error { margin: 0; color: var(--danger); max-width: 44em; }
</style>
