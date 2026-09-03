<script lang="ts">
  import type { PlacedRef, RefGroups } from '../state/refs.svelte';

  const { groups, head, onSelect }: {
    groups: RefGroups;
    head: string | null;
    onSelect: (row: number) => void;
  } = $props();

  let filter = $state('');
  // Remote and tag lists run to hundreds on a real repository, so they start closed as they do
  // in the reference; local branches are what people look at.
  let collapsed = $state<Record<string, boolean>>({ remote: true, tags: true });

  function shown(refs: PlacedRef[]): PlacedRef[] {
    const q = filter.trim().toLowerCase();
    return q ? refs.filter((r) => r.short.toLowerCase().includes(q)) : refs;
  }

  const sections = $derived([
    { key: 'local', title: 'Local', icon: '🖿', refs: shown(groups.local) },
    { key: 'remote', title: 'Remote', icon: '☁', refs: shown(groups.remote) },
    { key: 'stashes', title: 'Stashes', icon: '⤓', refs: shown(groups.stashes) },
    { key: 'tags', title: 'Tags', icon: '🏷', refs: shown(groups.tags) },
  ]);

  const total = $derived(
    groups.local.length + groups.remote.length + groups.tags.length + groups.stashes.length,
  );
</script>

<aside>
  <p class="viewing">Viewing <strong>{total}</strong></p>
  <input class="filter" placeholder="Filter" bind:value={filter} />
  {#each sections as section (section.key)}
    <section>
      <button class="head" onclick={() => (collapsed[section.key] = !collapsed[section.key])}>
        <span class="caret">{collapsed[section.key] ? '›' : '⌄'}</span>
        <span class="icon" aria-hidden="true">{section.icon}</span>
        {section.title}
        <span class="count">{section.refs.length}</span>
      </button>
      {#if !collapsed[section.key]}
        <ul>
          {#each section.refs.slice(0, 200) as r (r.name)}
            <li>
              <button
                class="ref"
                class:current={r.short === head}
                disabled={r.row === null}
                onclick={() => r.row !== null && onSelect(r.row)}
                title={r.row === null ? 'not in the loaded graph' : r.name}
              >
                {r.short}
                {#if r.ahead > 0 || r.behind > 0}
                  <span class="track">+{r.ahead} −{r.behind}</span>
                {/if}
              </button>
            </li>
          {/each}
          {#if section.refs.length > 200}
            <li class="more">…and {section.refs.length - 200} more</li>
          {/if}
        </ul>
      {/if}
    </section>
  {/each}
</aside>

<style>
  aside {
    width: 240px; flex: 0 0 auto; overflow-y: auto;
    border-right: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-2);
  }
  .viewing {
    margin: var(--space-1) var(--space-1) var(--space-2);
    font-size: 12px; color: var(--fg-2);
  }
  .viewing strong { color: var(--fg-0); font-weight: 600; }
  .icon { width: 1.1em; }
  .filter {
    width: 100%; box-sizing: border-box; font: inherit; font-size: 12px;
    padding: var(--space-1) var(--space-2); margin-bottom: var(--space-2);
    border: 1px solid var(--border); border-radius: 3px;
    background: var(--bg-0); color: var(--fg-0);
  }
  .head {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; font: inherit; font-size: 11px; text-transform: uppercase;
    letter-spacing: 0.04em; color: var(--fg-2);
    background: none; border: 0; padding: var(--space-2) var(--space-1); cursor: pointer;
  }
  .count { margin-left: auto; color: var(--fg-2); }
  .caret { width: 1em; }
  ul { list-style: none; margin: 0 0 var(--space-2); padding: 0; }
  .ref {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; font: inherit; font-size: 12px;
    padding: 3px var(--space-2) 3px var(--space-4);
    background: none; border: 0; border-radius: 3px; cursor: pointer;
    color: var(--fg-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .ref:hover:not(:disabled) { background: var(--bg-3); }
  .ref:disabled { color: var(--fg-2); cursor: default; }
  .ref.current { color: var(--fg-0); font-weight: 600; }
  .track { margin-left: auto; font-size: 11px; color: var(--fg-2); }
  .more { padding: 3px var(--space-4); font-size: 11px; color: var(--fg-2); }
</style>
