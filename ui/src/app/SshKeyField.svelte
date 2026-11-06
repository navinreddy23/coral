<script lang="ts">
  import type { SshKey } from '../ipc/types';
  import type { KeyChoice } from '../state/ssh.svelte';
  import { baseName } from './path';

  const {
    keys,
    value,
    onChange,
    onChooseFile,
    label = 'SSH key',
  }: {
    /** The keys to offer: those in `~/.ssh` and any chosen from elsewhere before. */
    keys: SshKey[];
    /** The chosen private key's path, or '' for whatever the agent offers. */
    value: string;
    onChange: (path: string) => void;
    /** Opens the file picker. Null when it was closed without a choice. */
    onChooseFile: () => Promise<KeyChoice | null>;
    label?: string;
  } = $props();

  let problem = $state<string | null>(null);

  const chosen = $derived(keys.find((k) => k.path === value));

  function named(key: SshKey): string {
    return key.comment === '' ? baseName(key.path) : `${baseName(key.path)} · ${key.comment}`;
  }

  async function browse() {
    const got = await onChooseFile();
    if (got === null) return;
    if ('error' in got) {
      problem = got.error;
      return;
    }
    problem = null;
    onChange(got.key.path);
  }
</script>

<div class="field">
  <label>
    <span class="name">{label}</span>
    <!-- An explicit handler rather than `bind:value`: the options arrive with the answer and a
         binding re-selects from state after they render, which is how a picker ends up
         showing a choice nobody made. -->
    <select
      {value}
      title={value}
      onchange={(e) => {
        problem = null;
        onChange(e.currentTarget.value);
      }}
    >
      <option value="">Whatever the agent offers</option>
      {#each keys as key (key.path)}
        <option value={key.path}>{named(key)}</option>
      {/each}
      {#if value !== '' && chosen === undefined}
        <!-- Set somewhere else, by hand or on another machine, and not found here. -->
        <option {value}>{value}</option>
      {/if}
    </select>
    <button type="button" class="pick" onclick={() => void browse()}>Choose a key file…</button>
  </label>
  <!-- Indented by a box of its own, at the field's font size: an indent in em on the smaller
       text itself would not line up with the select. -->
  <div class="under">
    {#if problem !== null}
      <p class="problem" role="alert">{problem}</p>
    {:else if chosen?.tooOpen}
      <p class="problem" role="alert">
        Other users can read this file, so ssh will refuse it. Run <code>chmod 600</code> on
        it first.
      </p>
    {/if}
    {#if chosen !== undefined}
      <p class="where mono" title={chosen.path}>{chosen.path}</p>
    {/if}
  </div>
</div>

<style>
  .field { display: flex; flex-direction: column; gap: var(--space-1); background: inherit; }
  label { display: flex; align-items: center; gap: var(--space-3); background: inherit; }
  .name { flex: 0 0 auto; width: var(--key-name-width, 7em); color: var(--fg-2); background: inherit; }
  select {
    flex: 1 1 auto; min-width: 0; font: inherit; font-size: var(--text-base);
    padding: 5px var(--space-2); border-radius: var(--radius-1);
    border: 1px solid var(--border); background: var(--bg-0); color: var(--fg-0);
  }
  .pick {
    flex: 0 0 auto; font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: 4px var(--space-3); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .pick:hover { background: var(--bg-3); color: var(--fg-0); }
  .under {
    display: flex; flex-direction: column; gap: var(--space-1); background: inherit;
    margin-left: calc(var(--key-name-width, 7em) + var(--space-3));
  }
  .where, .problem { margin: 0; font-size: var(--text-sm); background: inherit; overflow-wrap: anywhere; }
  .where { color: var(--fg-2); }
  .problem { color: var(--danger); }
  .mono { font-family: var(--font-mono); font-variant-ligatures: none; }
</style>
