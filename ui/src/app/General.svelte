<script lang="ts">
  import { t } from '../state/locale.svelte';
  import { INTERVALS, intervalWords, type AutoFetchState } from '../state/autofetch.svelte';

  /** What Coral does for every repository, as opposed to what one repository asks for. */
  const { autoFetch }: { autoFetch: AutoFetchState } = $props();
</script>

<section class="pane">
  <h2>{t('prefs.pane.general')}</h2>

  <fieldset>
    <legend>Fetching</legend>
    <label class="field">
      <span class="name">Fetch automatically</span>
      <select
        value={String(autoFetch.minutes)}
        onchange={(e) => autoFetch.setMinutes(Number(e.currentTarget.value))}
      >
        {#each INTERVALS as minutes (minutes)}
          <option value={String(minutes)}>{intervalWords(minutes)}</option>
        {/each}
      </select>
    </label>
    <p class="hint">
      The repository in front of you, while the window has focus, so what the graph says about
      its remotes is never older than this. A fetch that runs on its own is not added to undo and
      says nothing unless it fails. A repository can ask for something else under Repository.
    </p>
  </fieldset>
</section>

<style>
  .pane {
    flex: 1; min-width: 0; overflow-y: auto; padding: var(--space-4) var(--space-5); background: var(--bg-0);
    display: flex; flex-direction: column; gap: var(--space-5); align-items: flex-start;
  }
  h2 { margin: 0; font-size: var(--text-lg); font-weight: var(--weight-semibold); }
  fieldset {
    margin: 0; padding: 0; border: 0;
    display: flex; flex-direction: column; gap: var(--space-2);
  }
  legend {
    padding: 0; font-size: var(--text-sm); font-weight: var(--weight-semibold); color: var(--fg-2);
    text-transform: uppercase; letter-spacing: var(--tracking-label);
  }
  .field { display: flex; align-items: center; gap: var(--space-3); }
  .field .name { width: 10em; color: var(--fg-1); }
  .field select {
    font: inherit; font-size: var(--text-base); padding: 3px var(--space-2);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .hint { margin: 0; max-width: 60ch; color: var(--fg-2); line-height: 1.55; background: var(--bg-0); }
</style>
