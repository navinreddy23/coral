<script lang="ts">
  import Icon from './Icon.svelte';
  import { openInBrowser, pickProgram } from '../ipc/commands';
  import type { SigningConfig, SigningFormat } from '../ipc/types';
  import { defaultProgram, type SigningState } from '../state/signing.svelte';

  const { signing }: { signing: SigningState } = $props();

  const FORMATS: { value: SigningFormat; label: string }[] = [
    { value: 'openpgp', label: 'OpenPGP' },
    { value: 'x509', label: 'X.509' },
    { value: 'ssh', label: 'SSH' },
  ];

  /**
   * The values the form is showing.
   *
   * A copy rather than the state itself: nothing is written until Save, so a half-typed
   * program name never becomes the one git runs.
   */
  let draft = $state<SigningConfig | null>(null);
  let passphrase = $state('');

  // Reloaded whenever the level changes or the engine answers, so the form always starts from
  // what that level actually holds rather than from the last thing typed at the other one.
  $effect(() => {
    const scopes = signing.scopes;
    const level = signing.level;
    if (!scopes) return;
    draft =
      level === 'app'
        ? { ...scopes.global }
        : {
            format: scopes.local.format ?? scopes.global.format,
            program: scopes.local.program ?? scopes.global.program,
            key: scopes.local.key ?? scopes.global.key,
            signCommits: scopes.local.signCommits ?? scopes.global.signCommits,
            signTags: scopes.local.signTags ?? scopes.global.signTags,
          };
  });

  /** Which fields this repository sets itself, so the form can mark them. */
  const overridden = $derived(signing.scopes?.local ?? null);

  /** Picks the signing program from disk, for a version that is not on PATH. */
  async function browse() {
    const chosen = await pickProgram();
    if (chosen !== null && draft) {
      draft.program = chosen;
      await signing.refreshKeys();
    }
  }

  function save() {
    if (!draft) return;
    if (signing.level === 'app') {
      void signing.saveApp(draft);
    } else {
      // Everything the repository is being told, as an override. Clearing one is done with
      // "Inherit from app settings", which is a different action and says so.
      void signing.saveRepo({
        format: draft.format,
        program: draft.program === '' ? null : draft.program,
        key: draft.key === '' ? null : draft.key,
        signCommits: draft.signCommits,
        signTags: draft.signTags,
      });
    }
  }
</script>

<section class="pane">
  <h2>Commit Signing</h2>

  <!--
    Which level is being edited. GitKraken keeps this per profile; a key belongs to a
    repository, so the repository is the default here and the app level is the fallback.
  -->
  <div class="level" role="group" aria-label="Which settings to edit">
    <button class:on={signing.level === 'repo'} onclick={() => (signing.level = 'repo')}>
      This repository
    </button>
    <button class:on={signing.level === 'app'} onclick={() => (signing.level = 'app')}>
      All repositories
    </button>
    {#if signing.level === 'repo'}
      <span class="note">
        {#if signing.inheriting}
          Inheriting everything from the app settings.
        {:else}
          Overriding the app settings.
          <button class="link" onclick={() => void signing.inherit()}>Inherit instead</button>
        {/if}
      </span>
    {/if}
  </div>

  {#if draft}
    <dl>
      <dt><label for="fmt">GPG Format</label></dt>
      <dd>
        <select
          id="fmt"
          value={draft.format}
          onchange={(e) => {
            if (draft) draft.format = e.currentTarget.value as SigningFormat;
            void signing.refreshKeys();
          }}
        >
          {#each FORMATS as f (f.value)}
            <option value={f.value}>{f.label}</option>
          {/each}
        </select>
        {#if overridden?.format !== null && signing.level === 'repo'}
          <span class="tag">set here</span>
        {/if}
      </dd>

      <dt><label for="prog">Program</label></dt>
      <dd>
        <span class="row">
          <input
            id="prog"
            type="text"
            bind:value={draft.program}
            placeholder={defaultProgram(draft.format)}
          />
          <button class="refresh" onclick={browse}>Browse…</button>
        </span>
        <p class="hint">
          Usually {defaultProgram(draft.format)}. Leave it empty to use whatever is on PATH, or
          give a full path to pick a particular one.
        </p>
      </dd>

      <dt><label for="key">Signing Key</label></dt>
      <dd>
        <span class="row">
          <select id="key" bind:value={draft.key}>
            <option value="">&lt;None&gt;</option>
            {#each signing.keys as k (k.id)}
              <option value={k.id}>
                {k.label}{k.expired ? ' — expired' : ''}
              </option>
            {/each}
            {#if draft.key !== '' && !signing.keys.some((k) => k.id === draft?.key)}
              <!-- A key the program cannot see: still shown, or selecting it would look like
                   it had been forgotten. -->
              <option value={draft.key}>{draft.key} — not in the keyring</option>
            {/if}
          </select>
          <button
            class="refresh"
            title="Look for keys again"
            disabled={signing.loadingKeys}
            onclick={() => void signing.refreshKeys()}
          >
            <Icon name="restart" size={13} />
          </button>
        </span>
        {#if signing.keys.some((k) => k.id === draft?.key && k.expired)}
          <p class="hint warn">
            This key has expired. Renewing it means extending its expiry with your key tool;
            git will not sign with it until you do.
            <button
              class="link"
              onclick={() => void openInBrowser('https://docs.github.com/authentication/managing-commit-signature-verification')}
            >
              How do I renew an expired key?
            </button>
          </p>
        {/if}
      </dd>

      {#if draft.format === 'openpgp'}
        <dt><label for="pass">Generate new key</label></dt>
        <dd>
          <span class="row">
            <input
              id="pass"
              type="password"
              bind:value={passphrase}
              placeholder="Passphrase (optional)"
            />
            <button
              class="generate"
              disabled={signing.busy}
              onclick={() => {
                void signing.generate(draft?.program ?? '', passphrase);
                passphrase = '';
              }}
            >
              Generate
            </button>
          </span>
          <p class="hint">
            A 4096-bit key in the name and email this repository commits under, expiring in two
            years. It becomes the signing key for whichever level is selected above.
          </p>
        </dd>
      {/if}

      <dt>Sign commits</dt>
      <dd>
        <label class="check">
          <input type="checkbox" bind:checked={draft.signCommits} />
          Sign every commit made here
        </label>
      </dd>

      <dt>Sign tags</dt>
      <dd>
        <label class="check">
          <input type="checkbox" bind:checked={draft.signTags} />
          Sign every tag made here
        </label>
      </dd>
    </dl>

    <footer>
      {#if signing.error}
        <span class="error">{signing.error}</span>
      {:else if signing.done}
        <span class="done">{signing.done}</span>
      {:else if signing.effective}
        <span class="muted">
          Right now git {signing.effective.signCommits ? 'signs' : 'does not sign'} commits here.
        </span>
      {/if}
      <span class="spacer"></span>
      <button class="primary" disabled={signing.busy} onclick={save}>Save</button>
    </footer>
  {:else}
    <p class="muted">Reading the settings…</p>
  {/if}
</section>

<style>
  .pane { padding: var(--space-4) var(--space-5); overflow-y: auto; flex: 1; min-width: 0; }
  h2 { margin: 0 0 var(--space-4); font-size: var(--text-xl); font-weight: 600; color: var(--fg-0); }

  .level { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-4); }
  .level button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 2px var(--space-3);
    background: var(--bg-1); border: 1px solid var(--border); border-radius: var(--radius-1);
    color: var(--fg-1);
  }
  .level button.on {
    background: var(--accent); border-color: var(--accent); color: var(--accent-fg);
    font-weight: 600;
  }
  .note { font-size: var(--text-sm); color: var(--fg-2); }

  dl {
    display: grid; grid-template-columns: fit-content(30%) minmax(0, 1fr);
    gap: var(--space-3) var(--space-4); margin: 0; align-items: baseline;
  }
  dt { text-align: right; color: var(--fg-1); font-size: var(--text-base); }
  dd { margin: 0; min-width: 0; }
  .row { display: flex; gap: var(--space-2); align-items: center; }

  input, select {
    font: inherit; font-size: var(--text-base); padding: 3px var(--space-2);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0); min-width: 0;
  }
  /* Only the text fields. A bare `input` rule stretches the checkboxes to 300px too, which
     pushes their labels halfway across the pane. */
  input[type='text'], input[type='password'] { width: 300px; max-width: 100%; }
  input[type='checkbox'] { width: auto; flex: 0 0 auto; }
  select { width: 310px; max-width: 100%; }
  input:focus, select:focus { border-color: var(--accent); outline: none; }

  .refresh, .generate {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: 3px var(--space-3);
    background: var(--bg-1); border: 1px solid var(--border-strong);
    border-radius: var(--radius-1); color: var(--fg-1);
  }
  .refresh:hover:not(:disabled), .generate:hover:not(:disabled) { background: var(--bg-2); }
  .generate { color: var(--ok); font-weight: 600; }
  button:disabled { opacity: 0.5; cursor: default; }

  .hint { margin: var(--space-1) 0 0; font-size: var(--text-sm); color: var(--fg-2); line-height: 1.5; max-width: 46em; }
  .hint.warn { color: var(--warn); }
  .tag {
    margin-left: var(--space-2); font-size: var(--text-xs); padding: 0 6px; border-radius: 999px;
    background: var(--accent-soft); color: var(--accent);
  }
  .check { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-base); color: var(--fg-1); }

  .link {
    font: inherit; font-size: inherit; cursor: pointer; padding: 0;
    background: var(--bg-1); border: 0; color: var(--accent); text-decoration: underline;
  }

  footer {
    display: flex; align-items: center; gap: var(--space-3);
    margin-top: var(--space-5); padding-top: var(--space-3);
    border-top: 1px solid var(--border); font-size: var(--text-base);
  }
  .spacer { flex: 1; }
  .muted { color: var(--fg-2); }
  .done { color: var(--ok); }
  .error { color: var(--danger); }
  .primary {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: 3px var(--space-4);
    background: var(--accent); border: 1px solid var(--accent); border-radius: var(--radius-1);
    color: var(--accent-fg); font-weight: 600;
  }
  .primary:hover:not(:disabled) { background: var(--accent-hover); }
</style>
