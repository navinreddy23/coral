<script lang="ts">
  import CommitSigning from './CommitSigning.svelte';
  import Experimental from './Experimental.svelte';
  import Ssh from './Ssh.svelte';
  import type { ExperimentalState } from '../state/experimental.svelte';
  import type { SigningState } from '../state/signing.svelte';
  import type { SshState } from '../state/ssh.svelte';

  const { signing, ssh, experimental, hasRepository, onClose, onCopied, onPickGit }: {
    signing: SigningState;
    ssh: SshState;
    experimental: ExperimentalState;
    /**
     * Whether a repository is open.
     *
     * Two of these panes are about one repository and have nothing to read without it. The
     * third is about Coral itself, and is exactly what someone whose git is too old to open
     * anything has come here for.
     */
    hasRepository: boolean;
    onClose: () => void;
    /** Says whether the clipboard took something, which a button cannot tell on its own. */
    onCopied: (ok: boolean, what: string) => void;
    onPickGit: () => void;
  } = $props();

  /**
   * Only the panes that exist.
   *
   * The reference lists a dozen; listing ones that do nothing would be worse than not listing
   * them, so the rest arrive with the settings they hold.
   */
  const panes = $derived([
    { id: 'ssh', label: 'SSH', glyph: '⛨', needsRepository: true },
    { id: 'signing', label: 'Commit Signing', glyph: '✎', needsRepository: true },
    { id: 'experimental', label: 'Experimental', glyph: '⚗', needsRepository: false },
  ].filter((pane) => hasRepository || !pane.needsRepository));

  // svelte-ignore state_referenced_locally
  let active = $state(hasRepository ? 'ssh' : 'experimental');

  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
      event.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="prefs">
  <nav>
    <button class="back" onclick={onClose}>← Close preferences</button>
    <p class="heading">Preferences</p>
    {#each panes as pane (pane.id)}
      <button class="pane" class:on={active === pane.id} onclick={() => (active = pane.id)}>
        <span class="glyph" aria-hidden="true">{pane.glyph}</span>{pane.label}
      </button>
    {/each}
  </nav>

  {#if active === 'signing'}
    <CommitSigning {signing} />
  {:else if active === 'ssh'}
    <Ssh {ssh} {onCopied} />
  {:else if active === 'experimental'}
    <Experimental {experimental} {onPickGit} />
  {/if}
</div>

<style>
  .prefs {
    position: absolute; inset: 0; z-index: 12; display: flex;
    background: var(--bg-0);
  }
  nav {
    width: 240px; flex: 0 0 auto; padding: var(--space-2);
    border-right: 1px solid var(--border); background: var(--bg-1);
  }
  .back {
    display: block; width: 100%; text-align: left; font: inherit; font-size: 12px;
    cursor: pointer; padding: var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-1); border: 0; color: var(--accent);
  }
  .back:hover { background: var(--bg-2); }
  .heading {
    margin: var(--space-4) var(--space-2) var(--space-2);
    font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em;
    color: var(--fg-2);
  }
  .pane {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; font: inherit; font-size: 12px; cursor: pointer;
    padding: 4px var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-1); border: 0; color: var(--fg-1);
  }
  .pane:hover { background: var(--bg-2); }
  .pane.on {
    background: var(--accent-soft); color: var(--fg-0); font-weight: 600;
    box-shadow: inset 2px 0 0 var(--accent-line);
  }
  .glyph { color: var(--fg-2); width: 1.1em; }
  .pane.on .glyph { color: var(--accent); }
</style>
