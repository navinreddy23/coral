<script lang="ts">
  import { open } from '../ipc/commands';
  import type { RepoInfo } from '../ipc/types';

  let info = $state<RepoInfo | null>(null);
  let error = $state<string | null>(null);

  async function load(path: string) {
    error = null;
    try {
      info = await open(path);
    } catch (e) {
      error = String(e);
      info = null;
    }
  }

  // M0 proves the round trip only; M5 replaces this with the real shell.
  void load('.');
</script>

<main>
  <h1>Coral</h1>
  {#if error}
    <p class="error">{error}</p>
  {:else if info}
    <dl>
      <dt>Repository</dt><dd class="mono">{info.path}</dd>
      <dt>git</dt><dd class="mono">{info.gitVersion}</dd>
      <dt>HEAD</dt><dd class="mono">{info.head.kind}{'name' in info.head ? ` ${info.head.name}` : ''}</dd>
      <dt>Commit graph</dt><dd>{info.commitGraph ? 'present' : 'absent'}</dd>
    </dl>
  {:else}
    <p>Opening…</p>
  {/if}
</main>

<style>
  main { padding: var(--space-5); }
  h1 { font-size: 18px; font-weight: 600; margin: 0 0 var(--space-4); color: var(--accent); }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: var(--space-2) var(--space-4); margin: 0; }
  dt { color: var(--fg-2); }
  dd { margin: 0; color: var(--fg-0); }
  .error { color: var(--danger); }
</style>
