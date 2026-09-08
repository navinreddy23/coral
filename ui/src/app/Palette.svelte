<script lang="ts">
  import { rankCommand } from './palette';

  /** One thing the palette can run. */
  export interface Command {
    id: string;
    label: string;
    /** Where it came from — "Branch", "Remote", "Stash" — shown to the right. */
    group: string;
    run: () => void;
  }

  const { commands, onClose }: { commands: Command[]; onClose: () => void } = $props();

  let query = $state('');
  let cursor = $state(0);
  let input = $state<HTMLInputElement | null>(null);

  const matches = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (q === '') return commands.slice(0, 60);
    const scored: { command: Command; score: number }[] = [];
    for (const command of commands) {
      const score = rankCommand(`${command.group} ${command.label}`.toLowerCase(), q);
      if (score !== null) scored.push({ command, score });
    }
    scored.sort((a, b) => a.score - b.score);
    return scored.slice(0, 60).map((s) => s.command);
  });

  $effect(() => {
    void query;
    cursor = 0;
  });

  $effect(() => {
    input?.focus();
  });

  function choose(command: Command | undefined) {
    if (!command) return;
    onClose();
    command.run();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') onClose();
    else if (event.key === 'Enter') choose(matches[cursor]);
    else if (event.key === 'ArrowDown') cursor = Math.min(matches.length - 1, cursor + 1);
    else if (event.key === 'ArrowUp') cursor = Math.max(0, cursor - 1);
    else return;
    event.preventDefault();
  }
</script>

<div class="scrim" role="presentation" onclick={onClose}>
  <!-- Stops a click inside the panel reaching the scrim and closing it. -->
  <div class="panel" role="presentation" onclick={(e) => e.stopPropagation()}>
    <input
      bind:this={input}
      bind:value={query}
      onkeydown={onKey}
      placeholder="Type a command"
      aria-label="Command palette"
    />
    <ul>
      {#each matches as command, i (command.id)}
        <li>
          <button class:on={i === cursor} onclick={() => choose(command)}>
            <span class="label">{command.label}</span>
            <span class="group">{command.group}</span>
          </button>
        </li>
      {/each}
      {#if matches.length === 0}
        <li class="empty">No command matches.</li>
      {/if}
    </ul>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 20;
    background: var(--scrim);
    display: flex; justify-content: center; align-items: flex-start;
    padding-top: 12vh;
  }
  .panel {
    width: min(600px, 90vw); max-height: 60vh; display: flex; flex-direction: column;
    background: var(--bg-0); border: 1px solid var(--border-strong);
    border-radius: var(--radius-2); box-shadow: var(--elevate-2);
    overflow: hidden;
  }
  /*
   * A line to type on rather than a field to fill in. There is nothing else on this panel to
   * be confused with, and a bordered box inside a bordered panel is one edge too many.
   */
  input {
    font: inherit; font-size: var(--text-lg); padding: var(--space-3) var(--space-4);
    border: 0; border-bottom: 1px solid var(--border);
    background: var(--bg-0); color: var(--fg-0); outline: none;
  }
  input::placeholder { color: var(--fg-2); }
  ul { list-style: none; margin: 0; padding: var(--space-1); overflow-y: auto; }
  button {
    display: flex; align-items: center; gap: var(--space-3);
    width: 100%; text-align: left; cursor: pointer; font: inherit; font-size: var(--text-base);
    padding: 5px var(--space-2); background: var(--bg-0); border: 0;
    border-radius: var(--radius-1); color: var(--fg-1);
    transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  /* The row the arrow keys are on is filled; a hover only tints, so the two never look alike
     and the keyboard stays the thing driving the list. */
  button:hover { background: var(--bg-2); color: var(--fg-0); }
  button.on { background: var(--accent-soft); color: var(--fg-0); font-weight: 500; }
  .label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .group {
    flex: 0 0 auto; font-size: var(--text-xs); color: var(--fg-2);
    padding: 1px var(--space-2); border-radius: var(--radius-pill); background: var(--bg-2);
  }
  button.on .group { background: var(--bg-0); }
  .empty { padding: var(--space-4); font-size: var(--text-base); color: var(--fg-2); }
</style>
