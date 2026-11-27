<script lang="ts">
  import Icon from './Icon.svelte';

  /**
   * The × that closes or removes something, drawn the same everywhere.
   *
   * There were twenty-one, at five sizes, three of them a typed character, and most in the same
   * grey as the buttons beside them: the one that closes a diff could not be told from the
   * arrows next to it. Red sets it apart, as it does on a desktop's own window controls; red
   * already means "this ends something" here, and closing a view is that.
   */
  const {
    label,
    onclick,
    quiet = false,
    small = false,
  }: {
    /** What it closes, read out and shown on hover, since the glyph says only "close". */
    label: string;
    onclick: (event: MouseEvent) => void;
    /**
     * Grey until pointed at or focused, for a × repeated down a row of tabs, chips or
     * repositories, where red on every one would read as a list of things to delete.
     */
    quiet?: boolean;
    /** Inside a chip, which a full-size box would make twice as tall. */
    small?: boolean;
  } = $props();
</script>

<button class="close" class:quiet class:small aria-label={label} title={label} {onclick}>
  <Icon name="close" size={small ? 11 : 14} />
</button>

<style>
  .close {
    display: inline-flex; align-items: center; justify-content: center; flex: 0 0 auto;
    width: 24px; height: 24px; padding: 0;
    font: inherit; line-height: 1; cursor: pointer;
    background: none; border: 0; border-radius: var(--radius-1);
    color: var(--danger);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .quiet { color: var(--fg-2); }
  .close:hover, .close:focus-visible { background: var(--danger-soft); color: var(--danger); }
  .close:active { background: var(--danger); color: var(--bg-0); }
  .small { width: 16px; height: 16px; }
</style>
