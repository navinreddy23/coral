<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icon';
  import HostMark, { type HostMarkKind } from './HostMark.svelte';

  /**
   * The left panel minimised to its sections.
   *
   * Thirty-six pixels instead of two hundred and forty, which on a laptop is the difference
   * between reading a diff side by side and not. It is a way back rather than a second copy of
   * the panel: each icon opens the panel at its own section, so the branch list is one click
   * away instead of behind a keystroke nobody remembers.
   */
  const { counts, host, onOpen }: {
    /** How many rows each section holds, for the tooltip. Absent sections are not drawn. */
    counts: Record<string, number>;
    /** What the remotes point at, so the remote icon is the host's own mark. */
    host: HostMarkKind;
    /** Opens the panel with that section expanded and scrolled to. */
    onOpen: (section: string) => void;
  } = $props();

  interface Rung {
    key: string;
    title: string;
    icon: IconName;
    /** What one row of the section is called, for the tooltip's count. */
    each: string;
  }

  /** In the order the panel itself lists them, so the rail is a map of it. */
  const RUNGS: Rung[] = [
    { key: 'local', title: 'Local', icon: 'branch', each: 'branch' },
    { key: 'remote', title: 'Remote', icon: 'cloud', each: 'remote' },
    { key: 'stashes', title: 'Stashes', icon: 'stash', each: 'stash' },
    { key: 'tags', title: 'Tags', icon: 'tag', each: 'tag' },
    { key: 'prs', title: 'Requests', icon: 'request', each: 'request' },
    { key: 'submodules', title: 'Submodules', icon: 'folder', each: 'submodule' },
  ];

  const rungs = $derived(RUNGS.filter((rung) => counts[rung.key] !== undefined));

  function tip(rung: Rung): string {
    const n = counts[rung.key] ?? 0;
    return `${rung.title} — ${n} ${rung.each}${n === 1 ? '' : 's'}`;
  }
</script>

<nav class="rail" aria-label="Sections of the left panel">
  {#each rungs as rung (rung.key)}
    <button title={tip(rung)} aria-label={rung.title} onclick={() => onOpen(rung.key)}>
      {#if rung.key === 'remote'}
        <HostMark kind={host} />
      {:else}
        <Icon name={rung.icon} size={16} />
      {/if}
    </button>
  {/each}
</nav>

<style>
  /*
   * An opaque ground of its own, and the same one the panel it stands for wears. Every surface
   * carrying anything drawn has to paint its own background or WebKit stops knowing what is
   * behind it, and this one also has to read as the left edge rather than as part of the graph.
   */
  .rail {
    flex: 0 0 auto; display: flex; flex-direction: column; gap: var(--space-1);
    padding: var(--space-2) 0; width: 36px;
    background: var(--bg-1); border-right: 1px solid var(--border);
  }
  .rail button {
    display: flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; margin: 0 auto;
    border: 0; border-radius: var(--radius-1);
    background: none; color: var(--fg-2); cursor: pointer;
  }
  .rail button:hover { background: var(--bg-2); color: var(--fg-0); }
  .rail button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
</style>
