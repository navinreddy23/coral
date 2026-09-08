<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icon';
  import type { ThemeChoice, ThemeState } from '../state/theme.svelte';

  const { theme }: { theme: ThemeState } = $props();

  const CHOICES: { id: ThemeChoice; label: string; icon: IconName; note: string }[] = [
    {
      id: 'system',
      label: 'Follow the desktop',
      icon: 'monitor',
      note: 'Changes with it, while Coral is open.',
    },
    { id: 'light', label: 'Light', icon: 'sun', note: 'Whatever the desktop is doing.' },
    { id: 'dark', label: 'Dark', icon: 'moon', note: 'Whatever the desktop is doing.' },
  ];
</script>

<section class="pane">
  <h2>Appearance</h2>

  <fieldset>
    <legend>Theme</legend>
    <div class="choices">
      {#each CHOICES as choice (choice.id)}
        <button
          class="choice"
          class:on={theme.choice === choice.id}
          aria-pressed={theme.choice === choice.id}
          onclick={() => theme.set(choice.id)}
        >
          <span class="face"><Icon name={choice.icon} size={18} /></span>
          <span class="label">{choice.label}</span>
          <span class="note">{choice.note}</span>
        </button>
      {/each}
    </div>
    {#if theme.following}
      <p class="hint">
        The desktop is currently asking for {theme.current}. The switch in the title bar sets
        light or dark outright; this is how to hand the choice back.
      </p>
    {/if}
  </fieldset>
</section>

<style>
  .pane { display: flex; flex-direction: column; gap: var(--space-5); align-items: flex-start; }
  h2 { margin: 0; font-size: var(--text-lg); font-weight: 600; }

  fieldset {
    margin: 0; padding: 0; border: 0;
    display: flex; flex-direction: column; gap: var(--space-2);
  }
  legend {
    padding: 0; font-size: var(--text-sm); font-weight: 600; color: var(--fg-2);
    text-transform: uppercase; letter-spacing: 0.06em;
  }

  /* Three cards rather than a dropdown: there are exactly three, they are not going to grow,
     and a control that shows all of its options at once needs no second click to be read. */
  .choices { display: flex; gap: var(--space-2); flex-wrap: wrap; }
  .choice {
    display: flex; flex-direction: column; gap: var(--space-1); align-items: flex-start;
    width: 13.5em; padding: var(--space-3); text-align: left; cursor: pointer;
    font: inherit; color: var(--fg-1);
    background: var(--bg-1); border: 1px solid var(--border); border-radius: var(--radius-2);
    transition: background var(--fast) var(--ease), border-color var(--fast) var(--ease);
  }
  .choice:hover { background: var(--bg-2); border-color: var(--border-strong); }
  .choice.on {
    background: var(--accent-soft); border-color: var(--accent); color: var(--fg-0);
  }
  .face { color: var(--fg-2); }
  .choice.on .face { color: var(--accent); }
  .label { font-size: var(--text-md); font-weight: 600; color: var(--fg-0); }
  .note { font-size: var(--text-sm); color: var(--fg-2); line-height: var(--leading-body); }
  .hint {
    margin: 0; font-size: var(--text-sm); color: var(--fg-2);
    max-width: 42em; line-height: var(--leading-body);
  }
</style>
