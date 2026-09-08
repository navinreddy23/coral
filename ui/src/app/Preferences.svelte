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
  <!--
    A header with the way out at its trailing end, which is where every other panel that covers
    this window puts one and where the hand goes looking. It was a text link at the top of the
    side nav, reading as one more thing to open rather than as the way back.
  -->
  <header>
    <h2>Preferences</h2>
    {#if repoName !== ''}
      <span class="repo" title={repository}>{repoName}</span>
    {/if}
    <button class="shut" onclick={onClose} title="Close preferences (Esc)">
      <Icon name="close" size={15} />Close
    </button>
  </header>

  <div class="body">
  <nav>
    {#each panes as pane (pane.id)}
      <button class="pane" class:on={active === pane.id} onclick={() => (chosen = pane.id)}>
        <span class="glyph"><Icon name={pane.icon} size={14} /></span>{pane.label}
      </button>
    {/each}
  </nav>

  {#if active === 'appearance'}
    <Appearance {theme} {views} />
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
</div>

<style>
  .prefs {
    position: absolute; inset: 0; z-index: 12; display: flex; flex-direction: column;
    background: var(--bg-0);
  }
  /* Opaque, like every surface here that carries text: WebKit antialiases with subpixel
     precision only where it knows what is behind. */
  header {
    display: flex; align-items: center; gap: var(--space-3);
    flex: 0 0 auto; padding: var(--space-2) var(--space-3);
    background: var(--bg-1); border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0; font-size: var(--text-lg); font-weight: 600; color: var(--fg-0);
    flex: 0 0 auto;
  }
  /* Beside the title, so the two panes that are about one repository say which one before
     anything on them is read. */
  .repo {
    flex: 1; min-width: 0;
    font-size: var(--text-base); color: var(--fg-2);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /*
   * The word as well as the cross.
   *
   * Every other panel here closes on a bare cross, and that is right for one that overlays
   * part of the window: what is behind it says what closing goes back to. This one covers
   * everything, so there is nothing on screen to make sense of a lone glyph.
   */
  .shut {
    display: flex; align-items: center; gap: var(--space-1);
    margin-left: auto; flex: 0 0 auto; cursor: pointer;
    font: inherit; font-size: var(--text-base);
    padding: 4px var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .shut:hover { background: var(--bg-3); color: var(--fg-0); }
  .body { flex: 1; min-height: 0; display: flex; }
  nav {
    width: 240px; flex: 0 0 auto; padding: var(--space-2);
    border-right: 1px solid var(--border); background: var(--bg-1);
  }
  .pane {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; font: inherit; font-size: var(--text-base); cursor: pointer;
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
