<script lang="ts">
  const { shown }: { shown: boolean } = $props();
</script>

<!--
  Whether a branch is one the graph is drawn from.

  Lucide's eye and eye-off, redrawn at the size the sidebar uses, for the same reason as
  `RefMark.svelte`: a row is fourteen pixels of text and a word for this would not fit beside
  the name, the ahead/behind count and the dots.
-->
<svg
  class="eye"
  viewBox="0 0 24 24"
  width="12"
  height="12"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if shown}
    <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z" />
    <circle cx="12" cy="12" r="3" />
  {:else}
    <path d="M10.7 5.1A10.9 10.9 0 0 1 12 5c6.4 0 10 7 10 7a18.5 18.5 0 0 1-2.4 3.4" />
    <path d="M6.6 6.6A18.3 18.3 0 0 0 2 12s3.6 7 10 7a10.6 10.6 0 0 0 4.5-1" />
    <path d="M14.1 14.1a3 3 0 1 1-4.2-4.2" />
    <line x1="3" y1="3" x2="21" y2="21" />
  {/if}
</svg>

<style>
  .eye { flex: 0 0 auto; display: block; }
</style>
