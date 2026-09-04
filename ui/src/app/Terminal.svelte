<script lang="ts">
  import { onMount } from 'svelte';
  import { FitAddon } from '@xterm/addon-fit';
  import { WebLinksAddon } from '@xterm/addon-web-links';
  import { Terminal } from '@xterm/xterm';
  import '@xterm/xterm/css/xterm.css';

  import { openInBrowser } from '../ipc/commands';
  import {
    terminalListen,
    terminalResize,
    terminalWrite,
    type TerminalState,
  } from '../state/terminal.svelte';

  const { session, path, onClose }: {
    session: TerminalState;
    path: string;
    onClose: () => void;
  } = $props();

  let host = $state<HTMLDivElement | null>(null);
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let stop: (() => void) | null = null;

  /**
   * The palette, read from the page's own tokens.
   *
   * xterm draws to a canvas and cannot use CSS variables, so the theme has to be handed over
   * as literal colours — and re-handed whenever the theme changes, or the terminal stays light
   * in a dark window.
   */
  function palette(): Record<string, string> {
    const style = getComputedStyle(document.documentElement);
    const token = (name: string, fallback: string) =>
      style.getPropertyValue(name).trim() || fallback;
    return {
      background: token('--bg-0', '#ffffff'),
      foreground: token('--fg-0', '#171a20'),
      cursor: token('--accent', '#0d7a6f'),
      selectionBackground: token('--accent-soft', '#d6ece9'),
      black: token('--fg-0', '#171a20'),
      red: token('--danger', '#b3261e'),
      green: token('--ok', '#1a7f37'),
      yellow: token('--lane-3', '#8d6005'),
      blue: token('--lane-1', '#096cb3'),
      magenta: token('--lane-2', '#bf2681'),
      cyan: token('--lane-7', '#16737f'),
      white: token('--fg-1', '#4a5364'),
    };
  }

  async function send(size: { cols: number; rows: number }) {
    if (session.id !== null) await terminalResize(session.id, size.cols, size.rows);
  }

  onMount(() => {
    if (!host) return;
    term = new Terminal({
      fontFamily: getComputedStyle(document.documentElement)
        .getPropertyValue('--font-mono')
        .trim(),
      fontSize: 12,
      // Deep enough to scroll back through a long log or a rebase todo.
      scrollback: 10_000,
      cursorBlink: true,
      // A block cursor over software rendering costs nothing and is easier to find.
      cursorStyle: 'block',
      theme: palette(),
      allowProposedApi: true,
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    // A url printed by git — a merge request link, a hook's advice — opens in the browser
    // rather than doing nothing.
    term.loadAddon(new WebLinksAddon((_, uri) => void openInBrowser(uri)));
    term.open(host);
    fit.fit();

    void (async () => {
      const id = await session.start(path, term?.cols ?? 80, term?.rows ?? 24);
      if (id === null || !term) return;
      stop = await terminalListen(
        id,
        (text) => term?.write(text),
        () => {
          term?.writeln('\r\n\x1b[2m[the shell exited]\x1b[0m');
          void session.stop();
        },
      );
      term.onData((data) => void terminalWrite(id, data));
      term.focus();
    })();

    // The pane is resized by dragging its edge and by the window changing; both land here.
    const observer = new ResizeObserver(() => {
      fit?.fit();
      if (term) void send({ cols: term.cols, rows: term.rows });
    });
    observer.observe(host);

    return () => {
      observer.disconnect();
      stop?.();
      term?.dispose();
      term = null;
    };
  });

  // Re-themed rather than rebuilt: rebuilding would lose the scrollback and whatever is typed.
  $effect(() => {
    const theme = document.documentElement.dataset['theme'];
    void theme;
    if (term) term.options.theme = palette();
  });

  function copy() {
    const selected = term?.getSelection() ?? '';
    if (selected !== '') void navigator.clipboard.writeText(selected);
  }

  async function paste() {
    const text = await navigator.clipboard.readText();
    if (text !== '' && session.id !== null) await terminalWrite(session.id, text);
  }

  /**
   * Copy and paste on the shortcuts a terminal uses.
   *
   * Ctrl-C has to stay interrupt, which is why every terminal moves copy to Ctrl-Shift-C; the
   * webview would otherwise take both for its own.
   */
  function key(event: KeyboardEvent) {
    if (!event.ctrlKey || !event.shiftKey) return;
    if (event.key === 'C') {
      copy();
      event.preventDefault();
    } else if (event.key === 'V') {
      void paste();
      event.preventDefault();
    }
  }
</script>

<section class="terminal" class:right={session.dock === 'right'}>
  <header>
    <span class="title">Terminal</span>
    <span class="shell mono">{session.shell}</span>
    <span class="spacer"></span>
    <div class="dock" role="group" aria-label="Where the terminal sits">
      <button
        class:on={session.dock === 'bottom'}
        title="Dock to the bottom"
        onclick={() => session.setDock('bottom')}
      >
        ▁
      </button>
      <button
        class:on={session.dock === 'right'}
        title="Dock to the right"
        onclick={() => session.setDock('right')}
      >
        ▕
      </button>
    </div>
    <button class="close" onclick={onClose} aria-label="Hide the terminal">✕</button>
  </header>

  {#if session.error}
    <p class="error">{session.error}</p>
  {/if}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="host" bind:this={host} onkeydown={key} role="application" aria-label="Terminal"></div>
</section>

<style>
  .terminal {
    display: flex; flex-direction: column; min-width: 0; min-height: 0;
    background: var(--bg-0); border-top: 1px solid var(--border);
  }
  .terminal.right { border-top: 0; border-left: 1px solid var(--border); }
  header {
    display: flex; align-items: center; gap: var(--space-2);
    height: 24px; flex: 0 0 auto; padding: 0 var(--space-2);
    background: var(--bg-1); border-bottom: 1px solid var(--border);
    font-size: 11px; color: var(--fg-2);
  }
  .title { font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; }
  .shell { font-size: 10px; }
  .spacer { flex: 1; }
  .dock { display: flex; gap: 2px; }
  header button {
    font: inherit; cursor: pointer; padding: 0 var(--space-2); line-height: 18px;
    background: none; border: 0; border-radius: var(--radius-1); color: var(--fg-2);
  }
  header button:hover { background: var(--bg-2); color: var(--fg-0); }
  header button.on { background: var(--bg-3); color: var(--fg-0); }
  .error { margin: 0; padding: var(--space-2); font-size: 11px; color: var(--danger); }
  /* xterm measures itself against this box, so it must have a size of its own rather than
     being sized by the terminal it contains. */
  .host { flex: 1; min-height: 0; min-width: 0; padding: var(--space-1) var(--space-2); }
</style>
