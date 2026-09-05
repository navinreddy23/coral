<script lang="ts" module>
  import type { TabIcon } from '../state/tabs.svelte';

  /** Every picture a tab can carry, in the order the menu offers them. */
  export const TAB_ICONS: { id: TabIcon; label: string }[] = [
    { id: 'branch', label: 'Branch' },
    { id: 'github', label: 'GitHub' },
    { id: 'gitlab', label: 'GitLab' },
    { id: 'package', label: 'Package' },
    { id: 'terminal', label: 'Terminal' },
    { id: 'globe', label: 'Web' },
    { id: 'book', label: 'Docs' },
    { id: 'beaker', label: 'Experiment' },
    { id: 'wrench', label: 'Tools' },
    { id: 'star', label: 'Favourite' },
    { id: 'bug', label: 'Bugs' },
    { id: 'rocket', label: 'Release' },
  ];
</script>

<script lang="ts">
  import HostMark from './HostMark.svelte';

  const { kind, size = 14 }: { kind: TabIcon; size?: number } = $props();
</script>

<!--
  The picture on a tab, drawn rather than fetched.

  A browser reads a favicon off the network; a repository has no such thing, so this is the
  nearest equivalent a git client can offer: a small set of shapes that say what a checkout is
  for. Lucide's forms redrawn at fourteen pixels, `currentColor` throughout, so one copy works
  in both themes and takes whatever colour the tab gives it.
-->
{#if kind === 'github' || kind === 'gitlab'}
  <HostMark {kind} size={size - 1} />
{:else}
  <svg
    class="tab-mark"
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
    {#if kind === 'package'}
      <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" />
      <path d="m3.3 7 8.7 5 8.7-5" />
      <path d="M12 22V12" />
      <path d="m7.5 4.3 9 5.1" />
    {:else if kind === 'terminal'}
      <path d="m4 17 6-6-6-6" />
      <path d="M12 19h8" />
    {:else if kind === 'globe'}
      <circle cx="12" cy="12" r="10" />
      <path d="M2 12h20" />
      <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
    {:else if kind === 'book'}
      <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20" />
    {:else if kind === 'beaker'}
      <path d="M9 2v6.5L3.9 17A2 2 0 0 0 5.6 20h12.8a2 2 0 0 0 1.7-3L15 8.5V2" />
      <path d="M8 2h8" />
      <path d="M6.3 14h11.4" />
    {:else if kind === 'wrench'}
      <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
    {:else if kind === 'star'}
      <path d="m12 2.7 2.9 5.9 6.5.9-4.7 4.6 1.1 6.5-5.8-3-5.8 3 1.1-6.5L2.6 9.5l6.5-.9Z" />
    {:else if kind === 'bug'}
      <path d="M8 4a4 4 0 0 1 8 0" />
      <path d="M7 8h10a1 1 0 0 1 1 1v4a6 6 0 0 1-12 0V9a1 1 0 0 1 1-1Z" />
      <path d="M3 10h3M18 10h3M3 17h3M18 17h3M12 8v12" />
    {:else if kind === 'rocket'}
      <path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91 0Z" />
      <path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2Z" />
      <path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0" />
      <path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5" />
    {:else}
      <line x1="6" x2="6" y1="3" y2="15" />
      <circle cx="18" cy="6" r="3" />
      <circle cx="6" cy="18" r="3" />
      <path d="M18 9a9 9 0 0 1-9 9" />
    {/if}
  </svg>
{/if}

<style>
  .tab-mark { flex: 0 0 auto; display: block; }
</style>
