<script lang="ts">
  /**
   * Signing in to the host behind the repository's remote, as the profile at the window.
   *
   * A token belongs to a profile, so the same machine can hold a work account and a personal
   * one on the same host. The one exception is the token `coral host-login` writes, which has
   * no profile to belong to; a profile that has not signed in for itself falls back to it, and
   * this says so, because signing out of it signs every other profile out too.
   */
  import HostMark from './HostMark.svelte';
  import type { HostView } from '../ipc/commands';

  const {
    view,
    profile,
    shared,
    error,
    onSignIn,
    onSignOut,
    onClose,
  }: {
    view: HostView;
    /** The name of the profile at the window, which is who a new token is stored for. */
    profile: string;
    /** Whether the token in use is the shared one rather than this profile's own. */
    shared: boolean;
    error: string | null;
    onSignIn: (token: string) => void;
    onSignOut: () => void;
    onClose: () => void;
  } = $props();

  let token = $state('');
  let field = $state<HTMLInputElement | null>(null);

  const host = $derived(view.host);
  const label = $derived(host?.kind === 'gitlab' ? 'GitLab' : 'GitHub');
  const signedIn = $derived(view.token !== 'none');

  /**
   * Where the user makes one, on their own instance rather than on the public one.
   *
   * Both providers serve the page under the same path on a self-hosted install, so the origin
   * from the remote is enough and there is nothing to configure.
   */
  const makeUrl = $derived(
    host === null
      ? ''
      : host.kind === 'gitlab'
        ? `${host.origin}/-/user_settings/personal_access_tokens`
        : `${host.origin}/settings/tokens`,
  );
  const scope = $derived(host?.kind === 'gitlab' ? 'api' : 'repo');

  $effect(() => {
    field?.focus();
  });

  function submit() {
    const value = token.trim();
    if (value === '') return;
    token = '';
    onSignIn(value);
  }

  function key(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    onClose();
    event.preventDefault();
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim" role="presentation" onclick={onClose}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-label="{label} account"
    onclick={(e) => e.stopPropagation()}
  >
    <h2>
      <HostMark kind={host?.kind ?? 'other'} />
      {label}
    </h2>

    {#if host}
      <p class="where">{host.owner}/{host.repo} at {host.origin}</p>
    {/if}

    <!--
      The state in words rather than a tick. There are three of them and only one is about this
      profile alone, which is the whole point of the screen.
    -->
    <p class="state" class:on={signedIn}>
      {#if !signedIn}
        No token. Pull requests and their branches stay hidden until there is one.
      {:else if shared}
        Signed in for every profile, with the token the command line stored.
      {:else}
        Signed in as {profile}. No other profile can see this token.
      {/if}
    </p>

    <label class="field">
      <span>{signedIn ? 'Replace it with a token for ' + profile : 'Token for ' + profile}</span>
      <!--
        A password field, so a token does not sit in plain sight on a shared screen or in a
        screenshot, and so no password manager offers to remember it as a login.
      -->
      <input
        bind:this={field}
        bind:value={token}
        type="password"
        autocomplete="off"
        spellcheck="false"
        placeholder="paste it here"
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            submit();
            e.preventDefault();
          }
        }}
      />
    </label>

    {#if makeUrl}
      <p class="hint">
        Make one at <a href={makeUrl} target="_blank" rel="noreferrer">{makeUrl}</a> with the
        <code>{scope}</code> scope. It is kept in this machine's keyring, never in the repository.
      </p>
    {/if}

    {#if error !== null}<p class="bad">{error}</p>{/if}

    <div class="acts">
      {#if signedIn}
        <button class="out" onclick={onSignOut}>
          {shared ? 'Sign out everywhere' : `Sign ${profile} out`}
        </button>
      {/if}
      <button class="cancel" onclick={onClose}>Close</button>
      <button class="primary" disabled={token.trim() === ''} onclick={submit}>Sign in</button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 30; background: var(--scrim);
    display: flex; justify-content: center; align-items: flex-start; padding-top: 14vh;
  }
  .panel {
    width: min(480px, 92vw); padding: var(--space-4);
    background: var(--bg-0); border: 1px solid var(--border-strong);
    border-radius: var(--radius-2);
  }
  h2 {
    margin: 0; font-size: var(--text-lg); font-weight: 600; color: var(--fg-0);
    display: flex; align-items: center; gap: var(--space-2);
  }
  .where {
    margin: var(--space-1) 0 0; font-size: var(--text-base); color: var(--fg-2);
    font-family: var(--font-mono); font-variant-ligatures: none;
  }
  .state {
    margin: var(--space-3) 0 0; padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-1); font-size: var(--text-base); line-height: 1.5;
    background: var(--bg-1); color: var(--fg-2);
    border-left: 2px solid var(--border-strong);
  }
  .state.on { border-left-color: var(--accent); color: var(--fg-1); }
  .field {
    display: flex; flex-direction: column; gap: 4px;
    margin-top: var(--space-3); font-size: var(--text-base); color: var(--fg-2);
  }
  input {
    width: 100%; box-sizing: border-box; font: inherit; font-size: var(--text-md);
    padding: var(--space-2); border: 1px solid var(--border-strong);
    border-radius: var(--radius-1); background: var(--bg-0); color: var(--fg-0);
  }
  input:focus { border-color: var(--accent); outline: none; }
  .hint { margin: var(--space-2) 0 0; font-size: var(--text-sm); color: var(--fg-2); line-height: 1.6; }
  .hint a { color: var(--accent); }
  code {
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm);
    padding: 0 3px; border-radius: 3px; background: var(--bg-2);
  }
  .bad { margin: var(--space-2) 0 0; color: var(--danger); font-size: var(--text-base); }
  .acts {
    display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-4);
  }
  button {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: var(--space-1) var(--space-3);
    background: var(--bg-1); border: 1px solid var(--border-strong);
    border-radius: var(--radius-1); color: var(--fg-1);
  }
  button:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  button:disabled { opacity: 0.5; cursor: default; }
  .out { margin-right: auto; border-color: transparent; background: var(--bg-0); color: var(--danger); }
  .cancel { border-color: transparent; background: var(--bg-0); }
  .primary:not(:disabled) {
    background: var(--accent); border-color: var(--accent); color: var(--accent-fg);
    font-weight: 600;
  }
  .primary:hover:not(:disabled) { background: var(--accent-hover); }
</style>
