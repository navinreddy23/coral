<script lang="ts">
  import type { RefKind } from '../ipc/types';

  const { kind }: { kind: RefKind['kind'] } = $props();
</script>

<!--
  What sort of ref a label stands for, drawn rather than spelled.

  The sidebar says "TAGS" over its list and can afford a word; a pill eleven pixels tall cannot,
  and a row can carry a branch, a tag and a stash at once. Lucide's shapes, redrawn at the sizes
  used here, so a branch reads as a branch at twelve pixels.
-->
<svg
  class="ref-mark"
  viewBox="0 0 24 24"
  width="11"
  height="11"
  fill="none"
  stroke="currentColor"
  stroke-width="2.4"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if kind === 'tag'}
    <path d="M12.6 2.6A2 2 0 0 0 11.2 2H4a2 2 0 0 0-2 2v7.2a2 2 0 0 0 .6 1.4l8.7 8.7a2.4 2.4 0 0 0 3.4 0l6.6-6.6a2.4 2.4 0 0 0 0-3.4z" />
    <circle cx="7.5" cy="7.5" r="1.2" fill="currentColor" stroke="none" />
  {:else if kind === 'stash'}
    <path d="M12.8 2.2a2 2 0 0 0-1.6 0L2.6 6.1a1 1 0 0 0 0 1.8l8.6 3.9a2 2 0 0 0 1.6 0l8.6-3.9a1 1 0 0 0 0-1.8Z" />
    <path d="m6.1 11-3.5 1.6a1 1 0 0 0 0 1.8l8.6 3.9a2 2 0 0 0 1.6 0l8.6-3.9a1 1 0 0 0 0-1.8L17.9 11" />
  {:else}
    <line x1="6" x2="6" y1="3" y2="15" />
    <circle cx="18" cy="6" r="3" />
    <circle cx="6" cy="18" r="3" />
    <path d="M18 9a9 9 0 0 1-9 9" />
  {/if}
</svg>

<style>
  .ref-mark { flex: 0 0 auto; display: block; }
</style>
