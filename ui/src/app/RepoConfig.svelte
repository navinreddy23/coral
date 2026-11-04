<script lang="ts">
  import type { RepoConfigState } from '../state/repoconfig.svelte';

  /** The repository's own git switches: how it merges and, later, how fast its status is. */
  const { repoConfig }: { repoConfig: RepoConfigState } = $props();
  const config = $derived(repoConfig.config);
</script>

<section class="pane">
  <h2>Repository</h2>
  <p class="lede">
    Settings this repository keeps in its own git configuration, so git in a terminal follows
    them exactly as Coral does.
  </p>

  {#if repoConfig.error}
    <p class="error" role="alert">{repoConfig.error}</p>
  {/if}

  <h3>Merging</h3>
  <label class="check">
    <input
      type="checkbox"
      checked={config?.rerere ?? false}
      disabled={config === null || repoConfig.busy}
      onchange={(e) => void repoConfig.setRerere(e.currentTarget.checked)}
    />
    Reuse recorded resolutions
  </label>
  <p class="muted">
    git remembers how each conflict was resolved and resolves the same conflict the same way
    when it comes back, as it does on every rebase of a long-lived branch. The conflict tool
    marks a file it resolved this way, for you to check and keep. This is
    <code>rerere.enabled</code>.
  </p>

  <h3>Speed</h3>
  <p class="muted">
    Two caches git keeps in the index so a status need not look at every file. Neither changes
    what a status says, only how long it takes: on the Linux kernel's working tree the untracked
    cache alone takes a full status from about 215 ms to about 80 ms.
  </p>
  <label class="check">
    <input
      type="checkbox"
      checked={config?.caches.untrackedCache ?? false}
      disabled={config === null || repoConfig.busy}
      onchange={(e) => void repoConfig.setUntrackedCache(e.currentTarget.checked)}
    />
    Remember which folders hold nothing new
  </label>
  <p class="muted">The untracked cache, <code>core.untrackedCache</code>.</p>
  <label class="check" class:off={config !== null && !config.caches.fsmonitorSupported}>
    <input
      type="checkbox"
      checked={config?.caches.fsmonitor ?? false}
      disabled={config === null || repoConfig.busy || (!config.caches.fsmonitorSupported && !config.caches.fsmonitor)}
      onchange={(e) => void repoConfig.setFsmonitor(e.currentTarget.checked)}
    />
    Watch the working tree for changes
  </label>
  {#if config !== null && !config.caches.fsmonitorSupported}
    <p class="muted">
      This git was built without its filesystem monitor, as the usual Linux builds are, so
      there is nothing here to turn on. git for macOS and Windows has one.
    </p>
  {:else}
    <p class="muted">
      git's own monitor, <code>core.fsmonitor</code>: a small daemon per repository that tells
      git which paths changed since it last asked.
    </p>
  {/if}
</section>

<style>
  .pane {
    flex: 1; min-width: 0; overflow-y: auto; padding: var(--space-4) var(--space-5);
    background: var(--bg-0); color: var(--fg-1); font-size: var(--text-base);
  }
  h2 {
    margin: 0 0 var(--space-3); font-size: var(--text-xl); font-weight: var(--weight-semibold);
    color: var(--fg-0); background: var(--bg-0);
  }
  h3 {
    margin: var(--space-5) 0 var(--space-2); font-size: var(--text-lg);
    font-weight: var(--weight-semibold); color: var(--fg-0); background: var(--bg-0);
  }
  p { margin: 0 0 var(--space-3); max-width: 68ch; line-height: 1.55; background: var(--bg-0); }
  .lede { color: var(--fg-1); }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  code { font-family: var(--font-mono); font-variant-ligatures: none; }
  .check.off { color: var(--fg-2); }
  .check {
    display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2);
    color: var(--fg-0); background: var(--bg-0);
  }
</style>
