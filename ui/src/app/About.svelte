<script lang="ts">
  import { appVersion, openInBrowser, type AppVersion } from '../ipc/commands';
  import { messageOf } from '../ipc/error';
  import type { UpdateState } from '../state/update.svelte';

  const {
    onCopied,
    updates,
  }: { onCopied: (ok: boolean, what: string) => void; updates: UpdateState } = $props();

  let build = $state<AppVersion | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    appVersion()
      .then((v) => (build = v))
      .catch((e: unknown) => (error = messageOf(e)));
  });

  /**
   * The one line worth pasting into a bug report.
   *
   * A version on its own does not identify a build: hundreds carry the same one between two
   * releases, and a debug build is slow in ways worth knowing before anyone investigates a
   * complaint about speed.
   */
  const summary = $derived(
    build === null
      ? ''
      : `Coral ${build.number}${build.commit === null ? '' : ` (${build.commit})`}` +
        `${build.debug ? ' debug build' : ''}`,
  );

  const update = $derived(updates.result);
  const percent = $derived(updates.fraction === null ? null : Math.round(updates.fraction * 100));

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(summary);
      onCopied(true, 'the build');
    } catch {
      onCopied(false, 'the build');
    }
  }
</script>

<section>
  <h2>About</h2>
  {#if error !== null}
    <p class="bad">{error}</p>
  {:else if build === null}
    <p class="muted">Reading the build…</p>
  {:else}
    <dl>
      <dt>Version</dt>
      <dd class="mono">{build.number}</dd>

      <dt>Commit</dt>
      <dd class="mono">
        {#if build.commit === null}
          <span class="muted">not recorded, so this was built outside a checkout</span>
        {:else}
          {build.commit}
        {/if}
      </dd>

      <dt>Build</dt>
      <dd>{build.debug ? 'debug, which is several times slower than a release' : 'release'}</dd>
    </dl>

    <p class="line">
      <button onclick={() => void copy()}>Copy for a bug report</button>
      <span class="mono muted">{summary}</span>
    </p>

    <p class="line">
      <button
        onclick={() => void updates.check()}
        disabled={updates.checking || updates.installing}
      >
        {updates.checking ? 'Checking…' : 'Check for updates'}
      </button>
      {#if updates.installing}
        <span>
          Installing Coral {update?.latest}{percent === null ? '…' : `, ${percent}%`}
          It restarts when it is done.
        </span>
      {:else if updates.error !== null}
        <span class="bad">{updates.error}</span>
      {:else if update === null}
        <span class="muted">Asks GitHub for the newest release, only when pressed.</span>
      {:else if update.newer}
        <span>Coral {update.latest} is out.</span>
        {#if update.installable}
          <button class="primary" onclick={() => void updates.install()}>Install and restart</button>
        {/if}
        <button onclick={() => update && void openInBrowser(update.url)}>Open the release page</button>
      {:else if update.latest === update.current}
        <span class="muted">This is the newest release.</span>
      {:else}
        <span class="muted">This build is ahead of the newest release, {update.latest}.</span>
      {/if}
    </p>

    <label class="line">
      <input
        type="checkbox"
        checked={updates.automatic}
        onchange={(e) => updates.setAutomatic(e.currentTarget.checked)}
      />
      <span>Check for updates when Coral starts</span>
    </label>
  {/if}

  <p class="muted">
    A fast Git client. MIT licensed; see CREDITS.md for the projects it builds on.
  </p>
</section>

<style>
  section { padding: var(--space-4) var(--space-5); overflow: auto; flex: 1; }
  h2 { margin: 0 0 var(--space-3); font-size: var(--text-lg); }
  dl {
    display: grid; grid-template-columns: max-content 1fr;
    gap: var(--space-2) var(--space-3); margin: 0 0 var(--space-4); align-items: baseline;
  }
  dt {
    font-size: var(--text-xs); font-weight: var(--weight-semibold); text-transform: uppercase;
    letter-spacing: var(--tracking-label); color: var(--fg-2);
  }
  dd { margin: 0; font-size: var(--text-md); color: var(--fg-0); }
  .mono { font-family: var(--font-mono); font-variant-ligatures: none; }
  .muted { color: var(--fg-2); font-size: var(--text-base); }
  .bad { color: var(--danger); font-size: var(--text-base); }
  .line { display: flex; align-items: center; gap: var(--space-3); margin: 0 0 var(--space-4); }
  button {
    font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: 4px var(--space-3); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border-strong); color: var(--fg-0);
  }
  button:hover:not(:disabled) { background: var(--bg-3); }
  button.primary { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }
  button.primary:hover { background: var(--accent-hover); }
  label.line { font-size: var(--text-base); color: var(--fg-1); cursor: pointer; }
  button:disabled { cursor: default; color: var(--fg-2); }
</style>
