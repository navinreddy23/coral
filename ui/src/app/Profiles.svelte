<script lang="ts">
  import type { ProfilesState } from '../state/profiles.svelte';
  import type { GroupColour } from '../state/tabs.svelte';
  import type { IdentityScopes } from '../ipc/types';
  import { laneColour } from './lane';

  const { profiles, repository, identity, onSwitch, onDelete, onApplyHere }: {
    profiles: ProfilesState;
    /** The repository open in the active tab, or null when there is none. */
    repository: string | null;
    /** Who that repository commits as, so the pane can say whether it matches. */
    identity: IdentityScopes | null;
    onSwitch: (id: string) => void;
    /** Asks first: it closes every tab in the profile being removed. */
    onDelete: (id: string) => void;
    onApplyHere: () => void;
  } = $props();

  const COLOURS: GroupColour[] = [
    'lane1', 'lane2', 'lane3', 'lane4', 'lane5', 'lane6', 'lane7', 'lane8',
  ];

  let editing = $state<string | null>(null);
  let newName = $state('');

  /**
   * The fields being edited, kept beside what is stored.
   *
   * A draft rather than a live binding, for the reason the signing pane gives: half a typed
   * email address is not an identity anybody wants written into a repository, and each
   * keystroke would be a write.
   */
  let name = $state('');
  let email = $state('');
  let editedId = $state('');

  $effect(() => {
    const profile = profiles.current;
    if (profile.id === editedId) return;
    editedId = profile.id;
    name = profile.settings.user.name ?? '';
    email = profile.settings.user.email ?? '';
  });

  const dirty = $derived(
    name !== (profiles.current.settings.user.name ?? '') ||
      email !== (profiles.current.settings.user.email ?? ''),
  );

  /** What the repository would commit as, against what this profile says it should. */
  const matches = $derived.by(() => {
    if (identity === null) return true;
    const wanted = profiles.current.settings.user;
    if (wanted.name === null && wanted.email === null) return true;
    const has = identity.effective;
    return (
      (wanted.name === null || wanted.name === has.name) &&
      (wanted.email === null || wanted.email === has.email)
    );
  });

  function save() {
    const current = profiles.current;
    void profiles.setSettings(current.id, {
      ...current.settings,
      user: { name: name.trim() || null, email: email.trim() || null },
    });
  }

  function add() {
    const asked = newName.trim();
    if (asked === '') return;
    newName = '';
    void profiles.create(asked, COLOURS[profiles.all.length % COLOURS.length] ?? 'lane1');
  }

  function commitRename(id: string, to: string) {
    editing = null;
    if (to.trim() !== '') void profiles.rename(id, to);
  }

  /** The last segment, which is what people call a repository. */
  const repoName = $derived(repository?.split('/').filter(Boolean).pop() ?? '');
</script>

<section class="pane">
  <h2>Profiles</h2>
  <p class="lede">
    A profile holds the repositories you have open, the ones you opened recently, and the
    identity a repository cloned under it will carry. Work and personal, on one machine,
    without either showing while you are doing the other.
  </p>

  {#if profiles.error}<p class="error">{profiles.error}</p>{/if}

  <h3>Profiles</h3>
  <ul class="list">
    {#each profiles.all as profile (profile.id)}
      <li class:on={profile.id === profiles.currentId}>
        <span class="dot" style:--band={laneColour(profile.colour)} aria-hidden="true"></span>

        {#if editing === profile.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            autofocus
            value={profile.name}
            onblur={(e) => commitRename(profile.id, e.currentTarget.value)}
            onkeydown={(e) => {
              if (e.key === 'Enter') commitRename(profile.id, e.currentTarget.value);
              if (e.key === 'Escape') editing = null;
            }}
          />
        {:else}
          <span class="who">{profile.name}</span>
        {/if}

        {#if profile.id === profiles.currentId}
          <span class="badge">current</span>
        {:else}
          <button class="link" onclick={() => onSwitch(profile.id)}>Switch to</button>
        {/if}

        <span class="swatches">
          {#each COLOURS as colour (colour)}
            <button
              class="swatch"
              class:picked={colour === profile.colour}
              style:--band={laneColour(colour)}
              aria-label="Colour {colour}"
              onclick={() => void profiles.recolour(profile.id, colour)}
            ></button>
          {/each}
        </span>

        <button class="link" onclick={() => (editing = profile.id)}>Rename</button>
        <button
          class="link danger"
          disabled={profiles.all.length < 2}
          title={profiles.all.length < 2
            ? 'The last profile stays: its tabs have nowhere else to go.'
            : 'Removes the profile and the record of what it had open. The repositories are not touched.'}
          onclick={() => onDelete(profile.id)}
        >Remove</button>
      </li>
    {/each}
  </ul>

  <div class="field">
    <input
      class="new"
      bind:value={newName}
      placeholder="Work"
      onkeydown={(e) => e.key === 'Enter' && add()}
    />
    <button class="add" disabled={newName.trim() === ''} onclick={add}>Add a profile</button>
  </div>

  <h3>Identity for {profiles.current.name}</h3>
  <p class="muted">
    Written into a repository's own config when you clone or create one under this profile, so
    git and your terminal see exactly what Coral does. A repository you merely open is never
    changed without the button below.
  </p>

  <div class="field">
    <span class="name"><label for="who-name">Name</label></span>
    <input id="who-name" bind:value={name} placeholder="leave empty to inherit" />
  </div>
  <div class="field">
    <span class="name"><label for="who-email">Email</label></span>
    <input id="who-email" bind:value={email} placeholder="leave empty to inherit" />
  </div>
  <div class="field">
    <span class="name"></span>
    <button class="save" disabled={!dirty} onclick={save}>Save</button>
  </div>

  {#if repository !== null && identity !== null}
    <h3>This repository</h3>
    <p>
      <span class="mono">{repoName}</span> commits as
      <span class="mono">{identity.effective.name ?? 'no name'}</span>
      &lt;<span class="mono">{identity.effective.email ?? 'no email'}</span>&gt;.
    </p>
    {#if matches}
      <p class="muted">That is what this profile asks for.</p>
    {:else}
      <p class="muted">
        This profile asks for something else. Applying it writes the name, the email, the ssh
        key and the signing settings this profile sets into this repository alone.
      </p>
      <button class="save" onclick={onApplyHere}>Apply {profiles.current.name} here</button>
    {/if}
  {/if}
</section>

<style>
  .pane {
    flex: 1; min-width: 0; overflow-y: auto; padding: var(--space-4);
    background: var(--bg-0); color: var(--fg-1); font-size: var(--text-base);
  }
  h2 {
    margin: 0 0 var(--space-3); font-size: var(--text-xl); font-weight: 600; color: var(--fg-0);
    background: var(--bg-0);
  }
  h3 {
    margin: var(--space-5) 0 var(--space-2); font-size: var(--text-lg); font-weight: 600;
    color: var(--fg-0); background: var(--bg-0);
  }
  p { margin: 0 0 var(--space-3); max-width: 68ch; line-height: 1.55; background: var(--bg-0); }
  .lede { color: var(--fg-1); }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  .mono { font-family: var(--font-mono); font-variant-ligatures: none; }

  .list { list-style: none; margin: 0 0 var(--space-3); padding: 0; max-width: 68ch; }
  .list li {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 5px var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-0);
  }
  .list li.on { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line); }
  .dot { flex: 0 0 auto; width: 9px; height: 9px; border-radius: 50%; background: var(--band); }
  .who { flex: 1 1 auto; min-width: 0; color: var(--fg-0); font-weight: 600;
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge {
    font-size: var(--text-xs); font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.06em; color: var(--accent);
  }

  /* Eight small squares rather than a menu: the palette is the whole choice, and it is
     quicker to hit the colour than to open a list of eight names for it. */
  .swatches { display: inline-flex; gap: 2px; }
  .swatch {
    width: 12px; height: 12px; padding: 0; cursor: pointer;
    border: 1px solid transparent; border-radius: 3px; background: var(--band);
  }
  .swatch.picked { border-color: var(--fg-0); }

  .link {
    font: inherit; font-size: var(--text-sm); cursor: pointer;
    padding: 1px var(--space-2); border: 0; border-radius: var(--radius-1);
    background: transparent; color: var(--accent);
  }
  .link:hover:not(:disabled) { background: var(--bg-2); }
  .link:disabled { color: var(--fg-2); cursor: default; }
  .link.danger { color: var(--danger); }
  .link.danger:disabled { color: var(--fg-2); }

  .field { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-3); }
  .name { flex: 0 0 auto; width: 9em; color: var(--fg-2); }
  input {
    font: inherit; font-size: var(--text-base); padding: 3px var(--space-2); min-width: 0; max-width: 26em;
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .rename { flex: 1 1 auto; max-width: none; }
  .save, .add {
    font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: 4px var(--space-3); border-radius: var(--radius-1);
    border: 1px solid var(--border-strong); background: var(--bg-2); color: var(--fg-0);
  }
  .save:disabled, .add:disabled { opacity: 0.5; cursor: default; }
</style>
