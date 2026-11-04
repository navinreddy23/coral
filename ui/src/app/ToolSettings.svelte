<script lang="ts">
  import Icon from './Icon.svelte';
  import type { CustomAction } from '../ipc/commands';
  import type { ToolsState } from '../state/tools.svelte';

  /** The editor Coral hands a file to, the git tools beside it, and actions of the user's own. */
  const { tools }: { tools: ToolsState } = $props();
  const view = $derived(tools.view);
  const actions = $derived(view?.settings.actions ?? []);

  const ON: { value: CustomAction['on']; label: string }[] = [
    { value: 'repository', label: 'the repository' },
    { value: 'commit', label: 'a commit' },
    { value: 'file', label: 'a file' },
  ];

  /** An id nothing else holds, so a renamed or reordered action keeps its menu entries. */
  function freshId(): string {
    const taken = new Set(actions.map((a) => a.id));
    let n = actions.length + 1;
    while (taken.has(`action-${n}`)) n += 1;
    return `action-${n}`;
  }

  function change(index: number, patch: Partial<CustomAction>) {
    void tools.setActions(actions.map((a, i) => (i === index ? { ...a, ...patch } : a)));
  }

  /**
   * A new action being written, kept on screen until it has a name and a command, which is
   * when the engine keeps it. Three fields rather than one object: the inputs are bound to
   * them, and an object set to null under its own bindings is read once more as it goes.
   */
  let adding = $state(false);
  let draftName = $state('');
  let draftCommand = $state('');
  let draftOn = $state<CustomAction['on']>('commit');

  function saveDraft() {
    if (draftName.trim() === '' || draftCommand.trim() === '') return;
    void tools.setActions([
      ...actions,
      { id: freshId(), name: draftName.trim(), command: draftCommand.trim(), on: draftOn },
    ]);
    adding = false;
    draftName = '';
    draftCommand = '';
    draftOn = 'commit';
  }
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

  <h3>Your own actions</h3>
  <p class="muted">
    Commands of your own, run in the repository. One about the repository is in the command
    palette; one about a commit is in a commit's menu, and in the palette for the commit
    selected; one about a file is in a file's menu. <code>{'{commit}'}</code>,
    <code>{'{short}'}</code>, <code>{'{branch}'}</code>, <code>{'{file}'}</code>,
    <code>{'{path}'}</code> and <code>{'{repo}'}</code> are filled in, each as one argument.
    Nothing goes through a shell: for a pipe, write <code>sh -c '…'</code>. What it prints goes
    to the activity log.
  </p>
  <ul class="actions">
    {#each actions as action, i (action.id)}
      <li>
        <input
          class="label"
          value={action.name}
          aria-label="Name"
          onchange={(e) => change(i, { name: e.currentTarget.value })}
        />
        <select
          value={action.on}
          aria-label="About"
          onchange={(e) => change(i, { on: e.currentTarget.value as CustomAction['on'] })}
        >
          {#each ON as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
        </select>
        <input
          class="command"
          value={action.command}
          aria-label="Command"
          spellcheck="false"
          onchange={(e) => change(i, { command: e.currentTarget.value })}
        />
        <button
          class="remove"
          aria-label="Remove {action.name}"
          onclick={() => void tools.setActions(actions.filter((_, j) => j !== i))}
        ><Icon name="trash" size={13} /></button>
      </li>
    {/each}
    {#if adding}
      <li class="draft">
        <input class="label" bind:value={draftName} placeholder="Name" aria-label="Name" />
        <select bind:value={draftOn} aria-label="About">
          {#each ON as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
        </select>
        <input
          class="command"
          bind:value={draftCommand}
          placeholder="git log -1 --stat {'{commit}'}"
          aria-label="Command"
          spellcheck="false"
        />
        <button class="save" onclick={saveDraft}>Save</button>
      </li>
    {/if}
  </ul>
  {#if !adding}
    <button class="add" onclick={() => (adding = true)}>Add an action</button>
  {/if}
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
  .actions { list-style: none; margin: 0 0 var(--space-2); padding: 0; max-width: 60em; }
  .actions li { display: flex; gap: var(--space-2); align-items: center; margin-bottom: var(--space-2); }
  .actions input, .actions select {
    font: inherit; font-size: var(--text-base); padding: 3px var(--space-2);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .actions .label { flex: 0 0 12em; min-width: 0; }
  .actions .command { flex: 1 1 auto; min-width: 0; font-family: var(--font-mono); font-variant-ligatures: none; }
  .actions button, .add {
    flex: 0 0 auto; font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: 3px var(--space-3); background: var(--bg-1); color: var(--fg-1);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .actions .remove { padding: 3px var(--space-2); display: flex; align-items: center; }
  dl { display: grid; grid-template-columns: 9em 1fr; gap: var(--space-1) var(--space-3); margin: 0; }
  dt { color: var(--fg-2); }
  dd { margin: 0; color: var(--fg-0); font-family: var(--font-mono); }
</style>
