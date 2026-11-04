<script lang="ts">
  import type { ToolsState } from '../state/tools.svelte';

  /** The editor Coral hands a file to, and the git tools it can start beside it. */
  const { tools }: { tools: ToolsState } = $props();
  const view = $derived(tools.view);
</script>

<section class="pane">
  <h2>Tools</h2>
  <p class="lede">The programs a file's menu can open it in, outside Coral.</p>
  {#if tools.error}<p class="error" role="alert">{tools.error}</p>{/if}

  <h3>Editor</h3>
  <div class="field">
    <span class="name"><label for="editor">Command</label></span>
    <input
      id="editor"
      value={view?.settings.editor ?? ''}
      placeholder="the desktop's own choice for the file"
      spellcheck="false"
      onchange={(e) => void tools.setEditor(e.currentTarget.value)}
    />
  </div>
  <p class="muted">
    A program with a window of its own, such as <code>code --goto {'{path}'}:{'{line}'}</code>
    or <code>subl {'{path}'}:{'{line}'}</code>. <code>{'{path}'}</code> is the file,
    <code>{'{line}'}</code> the line, <code>{'{repo}'}</code> the repository; with neither path
    nor file named, the path goes at the end. It is started directly, not through a shell, so
    a file name is always one argument. A terminal editor belongs in the terminal pane.
  </p>

  <h3>Diff and merge tools</h3>
  <p class="muted">
    These are git's own, so a terminal beside Coral starts the same ones: set
    <code>diff.tool</code> and <code>merge.tool</code> (or the <code>guitool</code> keys) in
    your git configuration.
  </p>
  <dl>
    <dt>Diff tool</dt>
    <dd>{view?.external.diff ?? 'none set in this repository'}</dd>
    <dt>Merge tool</dt>
    <dd>{view?.external.merge ?? 'none set in this repository'}</dd>
  </dl>
</section>

<style>
  .pane {
    flex: 1; min-width: 0; overflow-y: auto; padding: var(--space-4) var(--space-5);
    background: var(--bg-0); color: var(--fg-1); font-size: var(--text-base);
  }
  h2 {
    margin: 0 0 var(--space-3); font-size: var(--text-xl); font-weight: var(--weight-semibold);
    color: var(--fg-0); background: var(--bg-0);
  }
  h3 {
    margin: var(--space-5) 0 var(--space-2); font-size: var(--text-lg);
    font-weight: var(--weight-semibold); color: var(--fg-0); background: var(--bg-0);
  }
  p { margin: 0 0 var(--space-3); max-width: 68ch; line-height: 1.55; background: var(--bg-0); }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  code { font-family: var(--font-mono); font-variant-ligatures: none; }
  .field { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-2); }
  .name { flex: 0 0 auto; width: 9em; color: var(--fg-2); }
  #editor {
    flex: 1 1 auto; min-width: 0; max-width: 34em; font: inherit; font-family: var(--font-mono);
    font-variant-ligatures: none; padding: 3px var(--space-2); background: var(--bg-0);
    color: var(--fg-0); border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  dl { display: grid; grid-template-columns: 9em 1fr; gap: var(--space-1) var(--space-3); margin: 0; }
  dt { color: var(--fg-2); }
  dd { margin: 0; color: var(--fg-0); font-family: var(--font-mono); }
</style>
