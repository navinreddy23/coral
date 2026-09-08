<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icon';
  import type { ThemeChoice, ThemeState } from '../state/theme.svelte';
  import type { Density, ViewsState } from '../state/views.svelte';
  import { ROW_HEIGHTS } from '../graph/layout';

  const { theme, views }: { theme: ThemeState; views: ViewsState } = $props();

  const CHOICES: { id: ThemeChoice; label: string; icon: IconName; note: string }[] = [
    {
      id: 'system',
      label: 'Follow the desktop',
      icon: 'monitor',
      note: 'Changes with it, while Coral is open.',
    },
    { id: 'light', label: 'Light', icon: 'sun', note: 'Always, whatever the desktop does.' },
    { id: 'dark', label: 'Dark', icon: 'moon', note: 'Always, whatever the desktop does.' },
  ];

  const DENSITIES: { id: Density; label: string; note: string }[] = [
    { id: 'compact', label: 'Compact', note: 'The most history on screen.' },
    { id: 'default', label: 'Default', note: 'What Coral ships with.' },
    { id: 'comfortable', label: 'Comfortable', note: 'More room around each row.' },
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

  <fieldset>
    <legend>Density</legend>
    <div class="choices">
      {#each DENSITIES as choice (choice.id)}
        <button
          class="choice"
          class:on={views.current.density === choice.id}
          aria-pressed={views.current.density === choice.id}
          onclick={() => views.set('density', choice.id)}
        >
          <span class="rows" aria-hidden="true" style:gap="{ROW_HEIGHTS[choice.id] / 8}px">
            {#each [0, 1, 2] as line (line)}
              <span class="rule"></span>
            {/each}
          </span>
          <span class="label">{choice.label}</span>
          <span class="note">{choice.note}</span>
        </button>
      {/each}
    </div>
    <p class="hint">
      Every row in the window follows this, not only the commit list: the lanes beside the
      messages are drawn on a canvas and are laid on the same grid.
    </p>
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
  /* Three rules at the height a row would be, which says what the choice does without anybody
     having to know what twenty-four pixels looks like. */
  .rows {
    display: flex; flex-direction: column; justify-content: center;
    height: 22px; width: 22px;
  }
  .rule { display: block; width: 100%; height: 3px; border-radius: 1px; background: var(--fg-2); }
  .choice.on .rule { background: var(--accent); }
  .label { font-size: var(--text-md); font-weight: 600; color: var(--fg-0); }
  .note { font-size: var(--text-sm); color: var(--fg-2); line-height: var(--leading-body); }
  .hint {
    margin: 0; font-size: var(--text-sm); color: var(--fg-2);
    max-width: 42em; line-height: var(--leading-body);
  }
</style>
