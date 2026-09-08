<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icon';
  import About from './About.svelte';
  import Appearance from './Appearance.svelte';
  import CommitSigning from './CommitSigning.svelte';
  import Experimental from './Experimental.svelte';
  import Profiles from './Profiles.svelte';
  import Ssh from './Ssh.svelte';
  import TerminalSettings from './TerminalSettings.svelte';
  import type { ExperimentalState } from '../state/experimental.svelte';
  import type { SigningState } from '../state/signing.svelte';
  import type { SshState } from '../state/ssh.svelte';
  import type { ProfilesState } from '../state/profiles.svelte';
  import type { ThemeState } from '../state/theme.svelte';
  import type { ViewsState } from '../state/views.svelte';
  import type { IdentityScopes } from '../ipc/types';

  const {
    signing,
    ssh,
    experimental,
    profiles,
    theme,
    views,
    identity,
    pane,
    repository,
    hasRepository,
    onClose,
    onCopied,
    onPickGit,
    onSwitchProfile,
    onDeleteProfile,
    onApplyProfileHere,
  }: {
    signing: SigningState;
    ssh: SshState;
    experimental: ExperimentalState;
    profiles: ProfilesState;
    theme: ThemeState;
    views: ViewsState;
    /** A pane to open on, when something asked for one rather than taking the default. */
    pane: string | null;
    /** Who the open repository commits as, for the Profiles pane to compare against. */
    identity: IdentityScopes | null;
    /**
     * The repository the two per-repository panes are describing.
     *
     * Named on the page rather than left to be inferred from the tab strip behind it: with two
     * repositories open, an ssh key on screen says nothing about whose it is.
     */
    repository: string | null;
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
    /** Switching closes every tab and opens the other profile's, so the window does it. */
    onSwitchProfile: (id: string) => void;
    onDeleteProfile: (id: string) => void;
    onApplyProfileHere: () => void;
  } = $props();

  /**
   * Only the panes that exist.
   *
   * The reference lists a dozen; listing ones that do nothing would be worse than not listing
   * them, so the rest arrive with the settings they hold.
   */
  const panes = $derived(([
    { id: 'appearance', label: 'Appearance', icon: 'sun', needsRepository: false },
    { id: 'profiles', label: 'Profiles', icon: 'blame', needsRepository: false },
    { id: 'ssh', label: 'SSH', icon: 'shield', needsRepository: true },
    { id: 'signing', label: 'Commit Signing', icon: 'edit', needsRepository: true },
    { id: 'terminal', label: 'Terminal', icon: 'terminal', needsRepository: false },
    { id: 'experimental', label: 'Experimental', icon: 'beaker', needsRepository: false },
    { id: 'about', label: 'About', icon: 'info', needsRepository: false },
  ] satisfies { id: string; label: string; icon: IconName; needsRepository: boolean }[]).filter(
    (pane) => hasRepository || !pane.needsRepository,
  ));

  // svelte-ignore state_referenced_locally
  let chosen = $state(pane ?? (hasRepository ? 'ssh' : 'experimental'));
  $effect(() => {
    if (pane !== null) chosen = pane;
  });

  /**
   * The pane on screen, which is the chosen one only while it still exists.
   *
   * Closing the last repository takes two panes off the list. A choice left pointing at one of
   * them rendered an empty page beside a nav with nothing selected.
   */
  const active = $derived(
    panes.some((pane) => pane.id === chosen) ? chosen : (panes[0]?.id ?? 'about'),
  );

  /** The last segment, which is what people call a repository. */
  const repoName = $derived(repository?.split('/').filter(Boolean).pop() ?? '');

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
    {#if repoName !== ''}
      <p class="repo" title={repository}>{repoName}</p>
    {/if}
    {#each panes as pane (pane.id)}
      <button class="pane" class:on={active === pane.id} onclick={() => (chosen = pane.id)}>
        <span class="glyph"><Icon name={pane.icon} size={14} /></span>{pane.label}
      </button>
    {/each}
  </nav>

  {#if active === 'appearance'}
    <Appearance {theme} />
  {:else if active === 'profiles'}
    <Profiles
      {profiles}
      {identity}
      {repository}
      onSwitch={onSwitchProfile}
      onDelete={onDeleteProfile}
      onApplyHere={onApplyProfileHere}
    />
  {:else if active === 'signing'}
    <CommitSigning {signing} />
  {:else if active === 'ssh'}
    <Ssh {ssh} {onCopied} />
  {:else if active === 'terminal'}
    <TerminalSettings {views} />
  {:else if active === 'experimental'}
    <Experimental {experimental} {onPickGit} />
  {:else if active === 'about'}
    <About {onCopied} />
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
  /* Under the heading, so the two panes that are about one repository say which one before
     anything on them is read. */
  .repo {
    margin: 0 var(--space-2) var(--space-2);
    font-size: 12px; font-weight: 600; color: var(--fg-1);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
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
  .glyph { color: var(--fg-2); display: flex; }
  .pane.on .glyph { color: var(--accent); }
</style>
