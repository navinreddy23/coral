<script lang="ts">
  import Icon from './Icon.svelte';
  import { coneOf, coveredBy } from './cone';
  import { repoFolders } from '../ipc/commands';
  import { messageOf } from '../ipc/error';

  /**
   * Which folders a sparse working tree should hold, picked from the repository's own tree.
   *
   * Read a level at a time as folders are opened: the kernel has five thousand of them and
   * nobody picking `drivers/gpu` wants the other four thousand nine hundred read first.
   */
  const {
    repo,
    initial,
    onApply,
    onClose,
  }: {
    repo: string;
    /** The folders the cone holds now. */
    initial: string[];
    onApply: (folders: string[]) => void;
    onClose: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let ticked = $state(new Set(initial));
  /** Each folder's children once read, by parent; `''` is the top. */
  let children = $state<Record<string, string[]>>({});
  let open = $state(new Set<string>());
  let error = $state<string | null>(null);

  async function read(under: string) {
    if (children[under] !== undefined) return;
    try {
      const got = await repoFolders(repo, under === '' ? null : under);
      children = { ...children, [under]: got };
    } catch (e) {
      error = messageOf(e);
    }
  }

  $effect(() => {
    void read('');
  });

  function toggleOpen(folder: string) {
    const next = new Set(open);
    if (next.has(folder)) next.delete(folder);
    else {
      next.add(folder);
      void read(folder);
    }
    open = next;
  }

  function tick(folder: string, on: boolean) {
    const next = new Set(ticked);
    if (on) next.add(folder);
    else next.delete(folder);
    ticked = next;
  }

  const chosen = $derived(coneOf(ticked));

  function name(folder: string): string {
    return folder.split('/').at(-1) ?? folder;
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
      event.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={key} />

{#snippet level(under: string, depth: number)}
  {#each children[under] ?? [] as folder (folder)}
    {@const inherited = coveredBy(folder, ticked)}
    <li style:padding-left="{depth * 18 + 8}px">
      <button
        class="twist"
        onclick={() => toggleOpen(folder)}
        aria-label={open.has(folder) ? `Close ${name(folder)}` : `Open ${name(folder)}`}
        aria-expanded={open.has(folder)}
      >
        <Icon name={open.has(folder) ? 'chevronDown' : 'chevronRight'} size={12} />
      </button>
      <label class:inherited>
        <input
          type="checkbox"
          checked={inherited || ticked.has(folder)}
          disabled={inherited}
          onchange={(e) => tick(folder, e.currentTarget.checked)}
        />
        <span class="mono">{name(folder)}</span>
      </label>
    </li>
    {#if open.has(folder)}
      {#if children[folder] === undefined}
        <li class="muted" style:padding-left="{(depth + 1) * 18 + 26}px">Reading…</li>
      {:else}
        {@render level(folder, depth + 1)}
      {/if}
    {/if}
  {/each}
{/snippet}

<div class="scrim" role="presentation" onclick={onClose}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div class="panel" role="dialog" aria-modal="true" aria-label="Choose folders" onclick={(e) => e.stopPropagation()}>
    <header>
      <h2>Folders to check out</h2>
      <p>
        A ticked folder brings everything inside it. The files at the top of the repository are
        always there.
      </p>
    </header>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <ul class="tree">
      {#if children[''] === undefined}
        <li class="muted">Reading the folders…</li>
      {:else}
        {@render level('', 0)}
      {/if}
    </ul>
    <footer>
      <span class="muted">
        {chosen.length === 0 ? 'Only the files at the top' : `${chosen.length} folder${chosen.length === 1 ? '' : 's'}`}
      </span>
      <span class="spacer"></span>
      <button onclick={onClose}>Cancel</button>
      <button class="primary" onclick={() => onApply(chosen)}>Check out these</button>
    </footer>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 30; background: var(--scrim);
    display: flex; justify-content: center; align-items: flex-start; padding-top: 8vh;
  }
  .panel {
    width: min(560px, 92vw); max-height: 80vh; display: flex; flex-direction: column;
    background: var(--bg-0); border: 1px solid var(--border-strong);
    border-radius: var(--radius-3); box-shadow: var(--elevate-2); overflow: hidden;
  }
  header { padding: var(--space-4) var(--space-4) var(--space-2); background: var(--bg-0); }
  h2 { margin: 0; font-size: var(--text-lg); font-weight: var(--weight-semibold); color: var(--fg-0); }
  header p { margin: var(--space-2) 0 0; color: var(--fg-2); line-height: 1.5; }
  .tree {
    list-style: none; margin: 0; padding: var(--space-1) 0; overflow-y: auto; flex: 1;
    border-top: 1px solid var(--border); border-bottom: 1px solid var(--border); background: var(--bg-0);
  }
  li { display: flex; align-items: center; gap: 2px; min-height: 24px; background: var(--bg-0); }
  li:hover { background: var(--bg-1); }
  .twist {
    flex: 0 0 auto; display: flex; align-items: center; justify-content: center;
    width: 18px; height: 18px; padding: 0; border: 0; background: none; cursor: pointer; color: var(--fg-2);
  }
  label { display: flex; align-items: center; gap: var(--space-2); cursor: pointer; color: var(--fg-0); }
  label.inherited { color: var(--fg-2); cursor: default; }
  .mono { font-family: var(--font-mono); font-size: var(--text-sm); }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); padding: 0 var(--space-4); }
  footer {
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-3) var(--space-4); background: var(--bg-1);
  }
  .spacer { flex: 1; }
  footer button {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: var(--space-1) var(--space-3);
    background: var(--bg-0); border: 1px solid var(--border-strong); border-radius: var(--radius-1); color: var(--fg-1);
  }
  footer button.primary {
    background: var(--accent); border-color: var(--accent); color: var(--accent-fg); font-weight: var(--weight-semibold);
  }
</style>
