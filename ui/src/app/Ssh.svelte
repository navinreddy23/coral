<script lang="ts">
  import { copyText } from './clipboard';
  import type { SshState } from '../state/ssh.svelte';

  const { ssh, onCopied }: {
    ssh: SshState;
    /** Says whether the clipboard took it, since the button cannot tell on its own. */
    onCopied: (ok: boolean, what: string) => void;
  } = $props();

  /**
   * The form, seeded from whichever level is being edited.
   *
   * Held separately from the loaded settings so a half-typed path is not written on every
   * keystroke, and so leaving the screen without saving changes nothing.
   */
  let useAgent = $state(true);
  let privateKey = $state('');
  let publicKey = $state('');
  let helper = $state('');
  let seeded = $state('');

  /**
   * Reseeds when the level or the loaded settings change, and not otherwise.
   *
   * Keyed on what was seeded from rather than run on every read: an effect that copied the
   * loaded values back into the form on each change would undo the user's typing.
   */
  $effect(() => {
    const scopes = ssh.scopes;
    if (!scopes) return;
    const key = `${ssh.level}:${JSON.stringify(scopes)}`;
    if (key === seeded) return;
    seeded = key;

    if (ssh.level === 'app') {
      useAgent = scopes.global.useAgent;
      privateKey = scopes.global.privateKey;
      publicKey = scopes.global.publicKey;
      helper = scopes.global.credentialHelper;
    } else {
      const local = scopes.local;
      useAgent = local.privateKey === null || local.privateKey === '';
      privateKey = local.privateKey ?? '';
      publicKey = local.publicKey ?? '';
      helper = local.credentialHelper ?? '';
    }
  });

  async function save() {
    if (ssh.level === 'app') {
      await ssh.saveApp({
        useAgent,
        privateKey: useAgent ? '' : privateKey.trim(),
        publicKey: publicKey.trim(),
        command: '',
        credentialHelper: helper.trim(),
      });
      return;
    }
    // An empty field at the repository level means "inherit", which is a different state from
    // an empty value; that difference is what lets an override be cleared.
    await ssh.saveRepo({
      privateKey: useAgent ? null : privateKey.trim() || null,
      publicKey: publicKey.trim() || null,
      credentialHelper: helper.trim() || null,
    });
  }

  /** Picking a listed key fills both halves, which is what people actually want. */
  function choose(path: string) {
    const found = ssh.keys.find((k) => k.path === path);
    if (!found) return;
    privateKey = found.path;
    publicKey = found.publicPath;
    useAgent = false;
  }

  async function copyPublic() {
    const text = await ssh.readPublic(publicKey);
    if (text === '') {
      onCopied(false, 'no public key to copy');
      return;
    }
    onCopied(await copyText(text), 'public key');
  }

  let newName = $state('id_coral');
  let newComment = $state('');
  let newPassphrase = $state('');
  let generating = $state(false);

  async function generate() {
    const key = await ssh.generate(newName.trim(), newComment.trim(), newPassphrase);
    if (key) {
      choose(key.path);
      generating = false;
      newPassphrase = '';
    }
  }

  const effective = $derived(ssh.effective);
</script>

<section class="pane">
  <header>
    <h2>SSH</h2>
    <div class="levels">
      <button class:on={ssh.level === 'repo'} onclick={() => (ssh.level = 'repo')}>
        This repository
      </button>
      <button class:on={ssh.level === 'app'} onclick={() => (ssh.level = 'app')}>
        Every repository
      </button>
    </div>
  </header>

  <!--
    What git will actually do, stated before anything can be changed. Two levels of
    configuration are only ever confusing without it.
  -->
  {#if effective}
    <p class="effective">
      {#if effective.useAgent}
        This repository uses the <strong>local ssh agent</strong>, which chooses the key.
      {:else}
        This repository signs in with <strong class="mono">{effective.privateKey}</strong>.
      {/if}
      {#if ssh.level === 'repo' && ssh.inheriting}
        <span class="tag">inherited</span>
      {:else if ssh.level === 'repo'}
        <span class="tag set">overridden here</span>
      {/if}
    </p>
  {/if}

  <label class="check">
    <input type="checkbox" bind:checked={useAgent} />
    Use the local ssh agent
  </label>
  <p class="hint">
    The agent offers whichever keys it holds. Turn this off to pin one key, which is what makes
    a repository push as the right account when the agent holds more than one. A pinned key is
    the only key: git is told to ignore <code>~/.ssh/config</code> here, since a key named there
    for the same host would otherwise be offered first and authenticate as the other account.
  </p>

  <label>
    SSH private key
    <span class="row">
      <input bind:value={privateKey} disabled={useAgent} spellcheck="false" placeholder="~/.ssh/id_ed25519" />
      {#if ssh.keys.length > 0}
        <select disabled={useAgent} onchange={(e) => choose(e.currentTarget.value)}>
          <option value="">Choose…</option>
          {#each ssh.keys as key (key.path)}
            <option value={key.path}>{key.path.split('/').pop()} · {key.kind}</option>
          {/each}
        </select>
      {/if}
    </span>
  </label>

  <label>
    SSH public key
    <span class="row">
      <input bind:value={publicKey} spellcheck="false" placeholder="~/.ssh/id_ed25519.pub" />
      <button disabled={publicKey.trim() === ''} onclick={copyPublic}>Copy it</button>
    </span>
  </label>
  <p class="hint">
    git never reads the public half. It is the one you paste into GitHub or GitLab.
  </p>

  <label>
    Git credential helper
    <input bind:value={helper} spellcheck="false" placeholder="leave empty to use Coral's own" />
  </label>
  <p class="hint">
    Empty means Coral answers git's credential prompts itself. Naming one here hands that back
    to the helper you already use, which is what a system keyring wants.
  </p>

  {#if ssh.error}<p class="error">{ssh.error}</p>{/if}
  {#if ssh.done}<p class="ok">{ssh.done}</p>{/if}

  <div class="buttons">
    <button class="primary" disabled={ssh.busy} onclick={save}>
      {ssh.level === 'app' ? 'Save for every repository' : 'Save for this repository'}
    </button>
    {#if ssh.level === 'repo' && !ssh.inheriting}
      <button disabled={ssh.busy} onclick={() => ssh.inherit()}>
        Clear the overrides and inherit
      </button>
    {/if}
    <button onclick={() => (generating = !generating)}>
      {generating ? 'Cancel' : 'Generate a new key pair'}
    </button>
  </div>

  {#if generating}
    <div class="generate">
      <h3>New key pair</h3>
      <p class="hint">
        An ed25519 key, written into your ssh directory. A passphrase is optional and cannot be
        added afterwards without regenerating.
      </p>
      <label>
        File name
        <input bind:value={newName} spellcheck="false" />
      </label>
      <label>
        Comment
        <input bind:value={newComment} spellcheck="false" placeholder="you@machine" />
      </label>
      <label>
        Passphrase
        <input type="password" bind:value={newPassphrase} />
      </label>
      <button class="primary" disabled={ssh.busy || newName.trim() === ''} onclick={generate}>
        Generate
      </button>
    </div>
  {/if}
</section>

<style>
  .pane {
    flex: 1; min-width: 0; overflow-y: auto;
    padding: var(--space-4) var(--space-5);
    display: flex; flex-direction: column; gap: var(--space-3); align-items: flex-start;
    font-size: var(--text-base);
  }
  header { display: flex; align-items: center; gap: var(--space-4); width: 100%; }
  h2 { margin: 0; font-size: var(--text-lg); font-weight: 600; flex: 1; }
  h3 { margin: 0; font-size: var(--text-base); font-weight: 600; }

  .levels { display: flex; border: 1px solid var(--border); border-radius: var(--radius-1);
            overflow: hidden; }
  .levels button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 3px var(--space-3);
    background: var(--bg-0); border: 0; color: var(--fg-2);
  }
  .levels button:hover { color: var(--fg-0); }
  .levels button.on { background: var(--accent); color: var(--accent-fg); font-weight: 600; }

  .effective {
    margin: 0; padding: var(--space-2) var(--space-3); width: 100%; box-sizing: border-box;
    background: var(--bg-2); border-radius: var(--radius-1); color: var(--fg-1);
    box-shadow: inset 2px 0 0 var(--accent-line);
  }
  .effective strong { color: var(--fg-0); }
  .tag {
    margin-left: var(--space-2); font-size: var(--text-xs); padding: 0 6px; border-radius: 999px;
    background: var(--bg-3); color: var(--fg-2);
  }
  .tag.set { background: var(--accent-soft); color: var(--accent); }

  label {
    display: flex; flex-direction: column; gap: var(--space-1);
    font-size: var(--text-sm); color: var(--fg-2); width: 100%; max-width: 44em;
  }
  .check { flex-direction: row; align-items: center; gap: var(--space-2);
           font-size: var(--text-base); color: var(--fg-0); }
  .row { display: flex; gap: var(--space-2); }
  .row input { flex: 1; min-width: 0; }
  input, select {
    font: inherit; font-size: var(--text-base);
    padding: var(--space-2); border: 1px solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  input:not([type]), input[type='password'] { font-family: var(--font-mono); font-variant-ligatures: none; }
  input:focus, select:focus { border-color: var(--accent); }
  input:disabled { background: var(--bg-2); color: var(--fg-2); }
  .hint { margin: 0; font-size: var(--text-sm); color: var(--fg-2); max-width: 44em; line-height: 1.5; }
  .error { margin: 0; color: var(--danger); }
  .ok { margin: 0; color: var(--ok); }

  .buttons { display: flex; gap: var(--space-2); flex-wrap: wrap; }
  .buttons button, .row button, .generate button {
    font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: var(--space-2) var(--space-3); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .buttons button:hover:not(:disabled), .row button:hover:not(:disabled) {
    background: var(--bg-3); color: var(--fg-0);
  }
  .buttons button:disabled, .row button:disabled { opacity: 0.5; cursor: default; }
  .primary {
    background: var(--accent); border-color: var(--accent); color: var(--accent-fg);
    font-weight: 600;
  }
  .primary:hover:not(:disabled) { background: var(--accent-hover); color: var(--accent-fg); }

  .generate {
    display: flex; flex-direction: column; gap: var(--space-2); align-items: flex-start;
    width: 100%; max-width: 44em; box-sizing: border-box;
    padding: var(--space-3); border: 1px solid var(--border); border-radius: var(--radius-2);
    background: var(--bg-1);
  }
</style>
