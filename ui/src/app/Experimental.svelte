<script lang="ts">
  import type { ExperimentalState } from '../state/experimental.svelte';
  import type { GitCandidate, GitChoice } from '../ipc/experimental';

  const { experimental, onPickGit }: {
    experimental: ExperimentalState;
    /** Asks for a git binary or an installation directory; the file picker is the window's. */
    onPickGit: () => void;
  } = $props();

  const view = $derived(experimental.view);

  /** A value the `<select>` can carry, since an option's value is a string. */
  function id(choice: GitChoice): string {
    return choice.kind === 'custom' ? `custom:${choice.path}` : choice.kind;
  }

  function label(candidate: GitCandidate): string {
    const version = candidate.version ?? 'will not run';
    switch (candidate.choice.kind) {
      case 'bundled':
        return `${version}: shipped with Coral`;
      case 'system':
        return `${version}: this machine's own git`;
      default:
        return `${version}: ${candidate.path}`;
    }
  }

  /**
   * What the control is showing, kept beside what is stored.
   *
   * `bind:value` rather than a one-way `value`: the select is populated from an answer that
   * arrives after it is drawn, and on the way there it showed the last option in the list as
   * though it were the setting.
   */
  let selected = $state('system');
  $effect(() => {
    if (view !== null) selected = id(view.chosen);
  });

  function choose(value: string) {
    if (value === 'browse') {
      onPickGit();
      return;
    }
    const found = view?.candidates.find((c) => id(c.choice) === value);
    if (found) void experimental.chooseGit(found.choice);
  }

  /** The candidate the engine is actually running, which is not always the one chosen. */
  const running = $derived(view?.candidates.find((c) => c.path === view.inUse) ?? null);
  const fellBack = $derived(
    view !== null && running !== null && running.choice.kind !== view.chosen.kind,
  );
</script>

<section>
  <h2>Experimental</h2>
  <p class="lede">
    The settings here are unfinished, and are offered so they can be used before they are. They
    may behave unexpectedly, and may change or be withdrawn. Everything on this page is
    optional; turning one off puts Coral back to how it behaves without it.
  </p>

  <h3>Git executable</h3>
  <p>
    Coral runs git itself for everything that writes, and needs version 2.40 or newer. Where a
    machine's own git is older than that, a build of Coral can carry one, and this is where to
    turn it on.
  </p>

  {#if experimental.error}
    <p class="error">{experimental.error}</p>
  {/if}

  {#if view === null}
    <p class="muted">Reading…</p>
  {:else}
    <label class="field">
      <span class="name">Git executable</span>
      <select bind:value={selected} disabled={experimental.busy} onchange={() => choose(selected)}>
        {#each view.candidates as candidate (id(candidate.choice))}
          <option value={id(candidate.choice)}>{label(candidate)}</option>
        {/each}
        <option value="browse">Choose a git…</option>
      </select>
    </label>

    {#each view.candidates as candidate (id(candidate.choice))}
      {#if candidate.problem && id(candidate.choice) === id(view.chosen)}
        <p class="error">{candidate.problem}</p>
      {/if}
    {/each}

    <p class="muted">
      In use: {view.inUseVersion ?? 'no git could be run'}
      {#if view.inUseVersion}at {view.inUse}{/if}
    </p>
    {#if fellBack}
      <p class="error">
        The git chosen here could not be used, so Coral fell back to the one above.
      </p>
    {/if}

    {#if view.candidates.some((c) => c.choice.kind === 'bundled')}
      <!-- Named because it has to be. git is somebody else's work under a licence that asks
           for the source to be offered, and a page that ships it without saying so is the
           wrong place to be quiet. -->
      <p class="note">
        The git shipped with Coral is an unmodified build of <strong>git</strong>, which is free
        software under the GNU General Public License, version 2. It is built from the release
        tarball published at
        <span class="mono">mirrors.edge.kernel.org/pub/software/scm/git</span>, and the exact
        version and its checksum are recorded in <span class="mono">SOURCE.txt</span> beside the
        binaries inside the bundle.
      </p>
    {/if}
  {/if}
</section>

<style>
  section {
    flex: 1; min-width: 0; overflow-y: auto; padding: var(--space-4);
    background: var(--bg-0); color: var(--fg-1); font-size: 12px;
  }
  h2 {
    margin: 0 0 var(--space-3); font-size: 18px; font-weight: 600; color: var(--fg-0);
    background: var(--bg-0);
  }
  h3 {
    margin: var(--space-5) 0 var(--space-2); font-size: 14px; font-weight: 600;
    color: var(--fg-0); background: var(--bg-0);
  }
  p { margin: 0 0 var(--space-3); max-width: 68ch; line-height: 1.55; background: var(--bg-0); }
  .lede { color: var(--fg-1); }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  /* The licence is a statement about what is being shipped, set apart from the instructions so
     it is not read as one of them. */
  .note {
    color: var(--fg-2); font-size: 11px; line-height: 1.6;
    border-left: 2px solid var(--border-strong); padding-left: var(--space-3);
  }
  .mono { font-family: var(--font-mono); }
  .field {
    display: flex; align-items: center; gap: var(--space-3);
    margin-bottom: var(--space-3);
  }
  .name { flex: 0 0 auto; width: 9em; color: var(--fg-2); }
  select {
    font: inherit; font-size: 12px; padding: 3px var(--space-2); min-width: 0; max-width: 40em;
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  select:disabled { opacity: 0.6; }
</style>
