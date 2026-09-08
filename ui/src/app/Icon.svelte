<script lang="ts">
  import { ICONS, strokeFor, type Glyph, type IconName } from './icon';

  const {
    name,
    size = 16,
    label,
  }: {
    name: IconName;
    size?: number;
    /**
     * What a screen reader should call it. Left off, the glyph is decoration: every control
     * in this window carries its own title or text, and announcing the picture as well would
     * say everything twice.
     */
    label?: string;
  } = $props();

  const glyph = $derived<Glyph>(ICONS[name]);
</script>

<!--
  One shape from the set, at whatever size the caller needs.

  The stroke is computed rather than fixed, so the same drawing keeps its optical weight from
  eleven pixels to twenty-four — see `strokeFor`. `currentColor` throughout, so a glyph takes
  the colour of whatever it sits inside and one copy serves both themes.
-->
<svg
  class="glyph"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width={strokeFor(size)}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden={label === undefined}
  role={label === undefined ? 'presentation' : 'img'}
>
  {#if label}<title>{label}</title>{/if}
  {#each glyph.paths ?? [] as d (d)}
    <path {d} />
  {/each}
  {#each glyph.solid ?? [] as d (d)}
    <path {d} fill="currentColor" stroke="none" />
  {/each}
  {#each glyph.dots ?? [] as [x, y, r] (`${x},${y}`)}
    <circle cx={x} cy={y} {r} fill="currentColor" stroke="none" />
  {/each}
</svg>

<style>
  /* Sits on the text baseline's centre rather than below it: every use of this is beside a
     word or inside a button, and a glyph that hangs low tilts the whole row. */
  .glyph { display: block; flex: 0 0 auto; }
</style>
