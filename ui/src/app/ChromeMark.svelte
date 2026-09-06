<script lang="ts" module>
  /** The window's own controls: the log, the preferences, and the two faces of the theme. */
  export type ChromeMarkKind = 'logs' | 'settings' | 'sun' | 'moon';
</script>

<script lang="ts">
  const { kind, size = 16 }: { kind: ChromeMarkKind; size?: number } = $props();
</script>

<!--
  The three buttons in the window's own chrome, which were a hamburger, a cog and the word
  "Dark" set at eleven pixels. Two of those are font glyphs, so their weight and their size
  came from whatever face the platform had for them rather than from anything here.

  Lucide's shapes, drawn at the same weight as every other mark in the window.
-->
<svg
  class="chrome-mark"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if kind === 'logs'}
    <path d="M3 5h1M3 12h1M3 19h1" />
    <path d="M8 5h13M8 12h13M8 19h13" />
  {:else if kind === 'settings'}
    <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
    <circle cx="12" cy="12" r="3" />
  {:else if kind === 'sun'}
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
  {:else}
    <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
  {/if}
</svg>

<style>
  .chrome-mark { display: block; flex: 0 0 auto; }
</style>
