<script lang="ts">
  import { t } from '../state/locale.svelte';
  import FolderPicker from './FolderPicker.svelte';
  import type { RepoConfigState } from '../state/repoconfig.svelte';
  import { intervalWords } from '../state/autofetch.svelte';
  import type { AutoFetch } from '../ipc/types';

  /** The repository's own git switches: how it merges, how fast its status is, what it holds. */
  const {
    repoConfig,
    windowMinutes,
  }: {
    repoConfig: RepoConfigState;
    /** The interval under General, which is what "as everywhere else" means. */
    windowMinutes: number;
  } = $props();
  const config = $derived(repoConfig.config);
  let picking = $state(false);

  /** A wider choice than General's, since one busy or one metered remote is the reason to set it. */
  const OWN_MINUTES = [1, 5, 10, 30, 60];

  const fetchChoice = $derived.by(() => {
    const own = config?.autoFetch ?? null;
    if (own === null) return 'window';
    return own.kind === 'off' ? 'off' : String(own.minutes);
  });

  function chooseFetch(value: string) {
    let to: AutoFetch | null = null;
    if (value === 'off') to = { kind: 'off' };
    else if (value !== 'window') to = { kind: 'every', minutes: Number(value) };
    void repoConfig.setAutoFetch(to);
  }
</script>

<section class="pane">
  <h2>{t('prefs.pane.repository')}</h2>
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

  <h3>Fetching</h3>
  <div class="field">
    <label for="auto-fetch" class="name">Automatically</label>
    <select
      id="auto-fetch"
      value={fetchChoice}
      disabled={config === null || repoConfig.busy}
      onchange={(e) => chooseFetch(e.currentTarget.value)}
    >
      <option value="window">As under General: {intervalWords(windowMinutes).toLowerCase()}</option>
      <option value="off">Never, for this repository</option>
      {#each OWN_MINUTES as minutes (minutes)}
        <option value={String(minutes)}>{intervalWords(minutes)}</option>
      {/each}
    </select>
  </div>
  <p class="muted">
    How often Coral fetches this repository on its own while it is the one in front of you. Kept
    in the repository's config as <code>coral.autoFetch</code>.
  </p>

  <h3>Issue links</h3>
  <div class="field">
    <label for="tracker" class="name">Tracker</label>
    <input
      id="tracker"
      value={config?.tracker ?? ''}
      placeholder="https://jira.example.com/browse/{'{key}'}"
      spellcheck="false"
      disabled={config === null || repoConfig.busy}
      onchange={(e) => void repoConfig.setTracker(e.currentTarget.value)}
    />
  </div>
  <p class="muted">
    Where <code>ABC-123</code> in a commit message links to, for issues kept somewhere other
    than the host: <code>{'{key}'}</code> is replaced by the reference. Issues on the host
    itself, <code>#12</code>, link without this. Kept in the repository's config as
    <code>coral.issueUrl</code>.
  </p>

  <h3>Sparse checkout</h3>
  {#if config === null}
    <p class="muted">Reading…</p>
  {:else if !config.sparse.enabled}
    <p class="muted">
      Every folder is checked out. A sparse checkout holds only the folders you choose and the
      files at the top, which on a very large repository is most of the disk and most of every
      status.
    </p>
  {:else if !config.sparse.cone}
    <p class="muted">
      This working tree is sparse by patterns rather than folders, set up outside Coral. It can
      be put back to everything here; choosing folders would replace the patterns.
    </p>
  {:else}
    <p>
      {config.sparse.folders.length === 0
        ? 'Only the files at the top are checked out.'
        : `${config.sparse.folders.length} folder${config.sparse.folders.length === 1 ? '' : 's'} and the files at the top:`}
    </p>
    {#if config.sparse.folders.length > 0}
      <ul class="folders">
        {#each config.sparse.folders as folder (folder)}<li><code>{folder}</code></li>{/each}
      </ul>
    {/if}
  {/if}
  {#if config !== null}
    <div class="row">
      <button disabled={repoConfig.busy} onclick={() => (picking = true)}>Choose folders…</button>
      {#if config.sparse.enabled}
        <button disabled={repoConfig.busy} onclick={() => void repoConfig.disableSparse()}>
          Check out everything
        </button>
      {/if}
    </div>
  {/if}
</section>

{#if picking && config !== null}
  <FolderPicker
    repo={repoConfig.path}
    initial={config.sparse.cone ? config.sparse.folders : []}
    onClose={() => (picking = false)}
    onApply={(folders) => {
      picking = false;
      void repoConfig.setSparse(folders);
    }}
  />
{/if}

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
  .folders { margin: 0 0 var(--space-3); padding-left: var(--space-4); max-height: 12em; overflow-y: auto; }
  .row { display: flex; gap: var(--space-2); }
  .field { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-2); }
  .field .name { flex: 0 0 auto; width: 6em; color: var(--fg-2); }
  #auto-fetch {
    font: inherit; padding: 3px var(--space-2); background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  #tracker {
    flex: 1 1 auto; min-width: 0; max-width: 34em; font: inherit; font-family: var(--font-mono);
    font-variant-ligatures: none; padding: 3px var(--space-2); background: var(--bg-0);
    color: var(--fg-0); border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .row button {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: var(--space-1) var(--space-3);
    background: var(--bg-1); border: 1px solid var(--border-strong); border-radius: var(--radius-1); color: var(--fg-1);
  }
  .row button:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  .check {
    display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2);
    color: var(--fg-0); background: var(--bg-0);
  }
</style>
