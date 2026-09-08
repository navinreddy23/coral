<script lang="ts">
  import type { ViewsState } from '../state/views.svelte';
  import { terminalDefaults, type TerminalDefaults } from '../ipc/terminal';

  /**
   * Which shell the pane runs, and how.
   *
   * The engine has always accepted a choice here and nothing ever made one, so everybody got
   * `$SHELL` started the same way whether or not that was the shell they use.
   */
  const { views }: { views: ViewsState } = $props();

  let usual = $state<TerminalDefaults | null>(null);
  $effect(() => {
    void terminalDefaults()
      .then((d) => (usual = d))
      .catch(() => (usual = null));
  });

  /** What the empty box will actually run, so the field is never a mystery. */
  const placeholder = $derived(usual === null ? 'the shell in $SHELL' : usual.shell);

  const login = $derived(views.current.terminalLogin ?? usual?.login ?? false);

  function setLogin(on: boolean) {
    // Stored as a decision once touched, rather than left following the platform: somebody who
    // turns it off on a Mac means it.
    views.set('terminalLogin', on);
  }
</script>

<section class="pane">
  <h2>Terminal</h2>
  <p class="lede">
    The shell Coral opens on <code>Ctrl+`</code>, in the repository the tab is showing. It is
    started interactively, so your prompt, aliases and completions are the ones you already
    have.
  </p>

  <div class="field">
    <span class="name"><label for="shell">Shell</label></span>
    <input
      id="shell"
      value={views.current.terminalShell}
      {placeholder}
      spellcheck="false"
      onchange={(e) => views.set('terminalShell', e.currentTarget.value)}
    />
  </div>
  <p class="muted">Leave it empty to use whatever this machine would.</p>

  <h3>Startup files</h3>
  <label class="check">
    <input type="checkbox" checked={login} onchange={(e) => setLogin(e.currentTarget.checked)} />
    Read the login files as well
  </label>
  <p class="muted">
    A login shell reads <code>.zprofile</code> and <code>.zlogin</code>, or
    <code>.bash_profile</code>, before the files an interactive shell reads. On macOS that is
    where <code>PATH</code> comes from and it is on by default. On Linux the desktop session
    has already read them, so doing it again only repeats what is in <code>PATH</code>.
  </p>
  {#if views.current.terminalLogin !== null}
    <p class="muted">
      Following your choice rather than the platform.
      <button class="link" onclick={() => views.set('terminalLogin', null)}>Use the default</button>
    </p>
  {/if}

  <p class="note">
    A shell already open keeps the settings it started with, because hiding the pane leaves it
    running and whatever is half-typed in it alone. The <span class="glyph">↻</span> button in
    the terminal starts it again with these.
  </p>
</section>

<style>
  .pane {
    flex: 1; min-width: 0; overflow-y: auto; padding: var(--space-4);
    background: var(--bg-0); color: var(--fg-1); font-size: 12px;
  }
  h2 {
    margin: 0 0 var(--space-3); font-size: 18px; font-weight: 600; color: var(--fg-0);
    background: var(--bg-0);
  }
  h3 {
    margin: var(--space-5) 0 var(--space-2); font-size: 14px; font-weight: 600;
    color: var(--fg-0); background: var(--bg-0);
  }
  p { margin: 0 0 var(--space-3); max-width: 68ch; line-height: 1.55; background: var(--bg-0); }
  .lede { color: var(--fg-1); }
  .muted { color: var(--fg-2); }
  .note {
    color: var(--fg-2); font-size: 11px; line-height: 1.6;
    border-left: 2px solid var(--border-strong); padding-left: var(--space-3);
  }
  code { font-family: var(--font-mono); }
  /* The button's own glyph, so the sentence points at something the eye can find. */
  .glyph {
    font-size: 12px; padding: 0 3px; border-radius: 3px;
    background: var(--bg-2); color: var(--fg-1);
  }
  .field { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-2); }
  .name { flex: 0 0 auto; width: 9em; color: var(--fg-2); }
  #shell {
    flex: 1 1 auto; min-width: 0; max-width: 34em;
    font: inherit; font-size: 12px; font-family: var(--font-mono);
    padding: 3px var(--space-2);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .check { display: inline-flex; align-items: center; gap: var(--space-2); cursor: pointer; }
  .link {
    font: inherit; font-size: 12px; cursor: pointer; padding: 0;
    background: none; border: 0; color: var(--accent); text-decoration: underline;
  }
</style>
