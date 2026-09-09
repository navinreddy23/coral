<script lang="ts">
  /**
   * One question, asked in the window rather than by the platform.
   *
   * `window.prompt` is styled by the webview, cannot be themed, and on some Linux webviews is
   * simply ignored — which turns "create a branch" into a button that does nothing at all.
   */
  export interface Choice {
    id: string;
    label: string;
    /** The one the Enter key takes. */
    primary?: boolean;
    /**
     * Whether answering this destroys something.
     *
     * Marked the way the menus mark the lines that open these questions, so the last thing
     * read before a tag goes, a branch goes or a change is discarded looks like what it is.
     */
    danger?: boolean;
  }

  const {
    title,
    detail = '',
    placeholder = '',
    initial = '',
    asksText = false,
    choices,
    onAnswer,
  }: {
    title: string;
    detail?: string;
    /**
     * Whether the question wants a line of text as well as a choice.
     *
     * Stated, not inferred. This was read off `placeholder !== ''`, so every question that
     * wanted text but had no hint to offer — rename this group, name this branch, reword this
     * commit — rendered no field at all and could only be cancelled.
     */
    asksText?: boolean;
    placeholder?: string;
    initial?: string;
    choices: Choice[];
    /** The chosen id and the typed text, or null when dismissed. */
    onAnswer: (choice: string | null, text: string) => void;
  } = $props();

  // Seeded once, then owned by the field. The component is created per question and
  // discarded with it, so there is no later `initial` to track.
  // svelte-ignore state_referenced_locally
  let text = $state(initial);
  let input = $state<HTMLInputElement | null>(null);

  /**
   * The choice the Enter key takes, if any.
   *
   * No fallback to the first choice. A question whose answer destroys something should have no
   * key that answers it: Enter is pressed to dismiss things, and a dialog that reads "this
   * cannot be undone" must not be one of them. Every caller that wants Enter says so.
   */
  const primary = $derived(choices.find((c) => c.primary) ?? null);

  $effect(() => {
    input?.focus();
    input?.select();
  });

  function answer(id: string | null) {
    onAnswer(id, text.trim());
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') answer(null);
    else if (event.key === 'Enter' && primary) answer(primary.id);
    else return;
    event.preventDefault();
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim" role="presentation" onclick={() => answer(null)}>
  <!--
    Escape and Enter are bound on the window, so the panel itself needs no key handler; the
    click handler exists only to stop a click inside it reaching the scrim and dismissing.
  -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    onclick={(e) => e.stopPropagation()}
  >
    <h2>{title}</h2>
    {#if detail}<p class="detail">{detail}</p>{/if}
    {#if asksText}
      <input bind:this={input} bind:value={text} {placeholder} aria-label={title} />
    {/if}
    <div class="choices">
      <button class="cancel" onclick={() => answer(null)}>Cancel</button>
      {#each choices as choice (choice.id)}
        <button
          class:primary={choice.id === primary?.id}
          class:danger={choice.danger === true}
          disabled={asksText && text.trim() === ''}
          onclick={() => answer(choice.id)}
        >
          {choice.label}
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 30; background: var(--scrim);
    display: flex; justify-content: center; align-items: flex-start; padding-top: 18vh;
  }
  .panel {
    width: min(440px, 90vw); padding: var(--space-4);
    background: var(--bg-0); border: 1px solid var(--border-strong);
    border-radius: var(--radius-2); box-shadow: var(--elevate-2);
  }
  h2 { margin: 0; font-size: var(--text-lg); font-weight: 600; color: var(--fg-0); }
  /* `pre-line`, so a detail can put the thing being destroyed on a line of its own and the
     warning on another. Run together they read as one sentence that says neither. */
  .detail {
    margin: var(--space-2) 0 0; font-size: var(--text-base); color: var(--fg-2); line-height: 1.5;
    white-space: pre-line;
  }
  input {
    width: 100%; box-sizing: border-box; font: inherit; font-size: var(--text-md);
    margin-top: var(--space-3); padding: var(--space-2);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
    background: var(--bg-0); color: var(--fg-0);
  }
  input:focus { border-color: var(--accent); outline: none; }
  .choices {
    display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-4);
  }
  button {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: var(--space-1) var(--space-3);
    background: var(--bg-1); border: 1px solid var(--border-strong);
    border-radius: var(--radius-1); color: var(--fg-1);
  }
  button:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  button:disabled { opacity: 0.5; cursor: default; }
  button.primary:not(:disabled) {
    background: var(--accent); border-color: var(--accent); color: var(--accent-fg);
    font-weight: 600;
  }
  button.primary:hover:not(:disabled) { background: var(--accent-hover); }
  /* Outlined rather than filled: a solid red button beside a grey Cancel reads as the one to
     press, and this is the one to think about. */
  button.danger:not(:disabled) {
    color: var(--danger); border-color: var(--danger); background: var(--bg-0);
  }
  button.danger:hover:not(:disabled) { background: var(--danger-soft); color: var(--danger); }
  .cancel { margin-right: auto; border-color: transparent; background: var(--bg-0); }
</style>
