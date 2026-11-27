<script lang="ts">
  import CodeLine from './CodeLine.svelte';
  import Close from './Close.svelte';
  import { hitSpans } from './hit';
  import { elidePath } from './path';
  import type { GrepMatch } from '../ipc/types';
  import type { GrepState } from '../state/grep.svelte';

  const { grep, onOpen, onClose }: {
    grep: GrepState;
    /** Opens a line the search found, in the file as the revision searched has it. */
    onOpen: (match: GrepMatch) => void;
    onClose: () => void;
  } = $props();

  // Focused and selected as the panel opens, so typing replaces the last search rather than
  // running on from the end of it.
  let input = $state<HTMLInputElement | null>(null);
  $effect(() => {
    input?.focus();
    input?.select();
  });

  const lines = $derived(grep.result?.matches.length ?? 0);
  const tally = $derived.by(() => {
    if (grep.result === null) return '';
    if (lines === 0) return 'Nothing matches.';
    const files = grep.files.length;
    const said = `${lines.toLocaleString()} line${lines === 1 ? '' : 's'} in ${files.toLocaleString()} file${files === 1 ? '' : 's'}`;
    return grep.result.more ? `The first ${said}. Narrow the search for the rest.` : `${said}.`;
  });

  function marks(text: string) {
    const asked = grep.asked;
    return asked === null ? null : hitSpans(text, asked.pattern, asked.how, asked.ignoreCase);
  }
</script>

<section class="grep">
  <header>
    <span class="title">Search code at <span class="at mono" title={grep.at?.rev}>{grep.at?.label}</span></span>
    <span class="spread"></span>
    <Close label="Close the code search (Esc)" onclick={onClose} />
  </header>
  <div class="ask">
    <input
      bind:this={input}
      class="pattern mono"
      placeholder="Text to find"
      aria-label="What to search for"
      spellcheck="false"
      value={grep.pattern}
      oninput={(e) => grep.set({ pattern: e.currentTarget.value })}
      onkeydown={(e) => {
        if (e.key === 'Enter') void grep.run();
        else if (e.key === 'Escape') onClose();
      }}
    />
    <button
      class="option mono"
      class:on={!grep.ignoreCase}
      aria-pressed={!grep.ignoreCase}
      title="Match case"
      onclick={() => grep.set({ ignoreCase: !grep.ignoreCase })}>Aa</button
    >
    <button
      class="option mono"
      class:on={grep.how === 'regex'}
      aria-pressed={grep.how === 'regex'}
      title="Read it as a regular expression"
      onclick={() => grep.set({ how: grep.how === 'regex' ? 'fixed' : 'regex' })}>.*</button
    >
    <input
      class="paths mono"
      placeholder="In paths: src *.c"
      aria-label="Where to search"
      spellcheck="false"
      value={grep.paths}
      oninput={(e) => grep.set({ paths: e.currentTarget.value })}
    />
  </div>
  <p class="status" role="status">
    {#if grep.searching}
      <span>Searching…</span>
      <button class="stop" onclick={() => grep.stop()}>Stop</button>
    {:else if grep.error !== null}
      <span class="error">{grep.error}</span>
    {:else if grep.result !== null}
      <span>{tally}</span>
    {:else}
      <span class="muted">Every file as this commit has it, binary ones left out.</span>
    {/if}
  </p>
  <div class="hits">
    {#each grep.files as file (file.path)}
      <div class="file">
        <span class="path mono" title={file.path}>{elidePath(file.path, 90)}</span>
        <span class="count">{file.matches.length}</span>
      </div>
      {#each file.matches as match (match.line)}
        <button class="hit" onclick={() => onOpen(match)} title="Open {file.path} at line {match.line}">
          <span class="no">{match.line}</span>
          <span class="text"><CodeLine text={match.text} spans={marks(match.text)} /></span>
        </button>
      {/each}
    {/each}
  </div>
</section>

<style>
  .grep {
    flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--bg-0);
    border: var(--hairline) solid var(--border); border-radius: var(--radius-3); overflow: hidden;
  }
  header {
    display: flex; align-items: center; gap: var(--space-2);
    height: 36px; padding: 0 var(--space-2) 0 var(--space-3); flex: 0 0 auto;
    border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  .title { font-size: var(--text-base); color: var(--fg-1); background: var(--bg-1); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .at { color: var(--fg-0); }
  .spread { flex: 1 1 auto; }

  .ask {
    display: flex; gap: var(--space-2); align-items: center; flex: 0 0 auto;
    padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--border); background: var(--bg-0);
  }
  .ask input {
    font: inherit; font-size: var(--text-sm); color: var(--fg-0); background: var(--bg-0);
    border: 1px solid var(--border); border-radius: var(--radius-1); padding: 3px var(--space-2);
  }
  .ask input:focus { outline: 2px solid var(--accent-line); outline-offset: -1px; }
  .pattern { flex: 2 1 0; min-width: 8em; }
  .paths { flex: 1 1 0; min-width: 6em; }
  .option {
    flex: 0 0 auto; min-width: 26px; height: 24px; font-size: var(--text-sm); cursor: pointer;
    background: none; border: 1px solid transparent; border-radius: var(--radius-1); color: var(--fg-2);
  }
  .option:hover { color: var(--fg-0); background: var(--bg-2); }
  .option.on { background: var(--accent-soft); color: var(--accent-ink); border-color: var(--accent-line); }

  .status {
    display: flex; gap: var(--space-2); align-items: center; margin: 0; flex: 0 0 auto;
    padding: var(--space-1) var(--space-3); font-size: var(--text-sm); color: var(--fg-1); background: var(--bg-0);
  }
  .status .muted { color: var(--fg-2); }
  .status .error { color: var(--danger); }
  .stop {
    font: inherit; font-size: var(--text-sm); cursor: pointer; background: var(--bg-0); color: var(--fg-1);
    border: 1px solid var(--border); border-radius: var(--radius-0); padding: 0 var(--space-2); line-height: 18px;
  }

  /* The pattern marked in the warm tint a search highlight wears everywhere else. */
  .hits { flex: 1; min-height: 0; overflow: auto; padding-bottom: var(--space-3); --word-mark: var(--warn-soft); }
  .file {
    position: sticky; top: 0; z-index: 1; display: flex; gap: var(--space-2); align-items: baseline;
    padding: var(--space-2) var(--space-3) var(--space-1); background: var(--bg-1);
    border-bottom: 1px solid var(--border);
  }
  .file .path { font-size: var(--text-sm); color: var(--fg-0); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file .count { font-size: var(--text-xs); color: var(--fg-2); }
  .hit {
    display: flex; width: 100%; align-items: baseline; gap: var(--space-2); text-align: left; cursor: pointer;
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); line-height: 17px;
    background: var(--bg-0); border: 0; padding: 1px var(--space-3); color: var(--fg-0);
  }
  .hit:hover { background: var(--bg-2); }
  .hit .no { flex: 0 0 6ch; text-align: right; color: var(--fg-2); font-variant-numeric: tabular-nums; }
  .hit .text { flex: 1; min-width: 0; white-space: pre; overflow: hidden; text-overflow: ellipsis; }
</style>
