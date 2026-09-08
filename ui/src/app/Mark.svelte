<script lang="ts">
  const { size = 18, label }: {
    size?: number;
    /** A name, where the mark stands alone. Left off it is decoration beside the wordmark. */
    label?: string;
  } = $props();

  /**
   * The stroke, kept at one optical weight across every size the mark is drawn at, the same
   * way the icon set does it. The mark appears at eighteen pixels in the title bar and at a
   * thousand and twenty-four in the application icon.
   */
  const stroke = $derived(Math.min(2.9, Math.max(1.7, 33 / size)));
</script>

<!--
  Coral's mark: one commit, the two branches that leave it, and the trunk carrying on past
  them.
  
  It is a commit graph and it is a piece of coral, which is the whole idea — a thing that grows
  by branching. Four nodes rather than three, because three made a Y, and a Y is what every
  other client already uses for a branch. Drawn in `currentColor`, so it takes the brand colour
  from whatever it is placed in and needs no second copy for the dark theme.
-->
<svg
  class="mark"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  aria-hidden={label === undefined}
  role={label === undefined ? 'presentation' : 'img'}
>
  {#if label}<title>{label}</title>{/if}
  <path d="M12 22v-8" />
  <path d="M12 14V4" />
  <path d="M12 14c0-3-2.5-3.6-5.5-5.2" />
  <path d="M12 14c0-3 2.5-3.6 5.5-5.2" />
  <circle cx="12" cy="22" r="2.3" fill="currentColor" stroke="none" />
  <circle cx="12" cy="3.6" r="2.3" fill="currentColor" stroke="none" />
  <circle cx="6" cy="8" r="2.3" fill="currentColor" stroke="none" />
  <circle cx="18" cy="8" r="2.3" fill="currentColor" stroke="none" />
</svg>

<style>
  .mark { display: block; flex: 0 0 auto; }
</style>
