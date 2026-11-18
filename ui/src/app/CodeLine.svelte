<script lang="ts">
  import type { Span } from '../diff/word';
  import { segmentsOf } from '../syntax/segment';
  import type { Token } from '../syntax/token';

  /**
   * One line of code, coloured by what it is and with the words that changed inside it marked.
   *
   * A component rather than a block in the panel because the markup has to carry no whitespace
   * of its own: the text is rendered under `white-space: pre`, so a newline between two tags
   * would show up as a space in the middle of the code.
   */
  const { text, spans, tokens = null }: {
    text: string;
    /** Null for a line with nothing to mark, which is most of them. */
    spans: Span[] | null;
    /** Null for a line drawn plain: colours off, the file too large, or not back yet. */
    tokens?: Token[] | null;
  } = $props();

  const segments = $derived(spans === null && tokens === null ? null : segmentsOf(text, tokens, spans));
</script>

{#if segments === null}{text}{:else}{#each segments as s, i (i)}{#if s.marked}<mark class={s.category === null ? undefined : `tk-${s.category}`}>{s.text}</mark>{:else if s.category !== null}<span class="tk-{s.category}">{s.text}</span>{:else}{s.text}{/if}{/each}{/if}

<style>
  /*
   * The browser default is black on yellow, which has nothing to do with a diff.
   *
   * The colour is the line's own, handed down as a property rather than set here: a scoped
   * rule in the panel cannot reach into this component, and the mark has to be one step of
   * whichever tint the line it sits on is wearing. No tint set means no mark, which is what a
   * line outside a diff gets.
   */
  mark {
    background: var(--word-mark, transparent); color: inherit;
    padding: 0; border-radius: var(--radius-0);
  }

  /* Colour only: JetBrains Mono ships a regular and a bold and nothing slanted, and weight is
     what a picked line already uses to stand out. */
  .tk-keyword { color: var(--syn-keyword); }
  .tk-string { color: var(--syn-string); }
  .tk-comment { color: var(--syn-comment); }
  .tk-constant { color: var(--syn-constant); }
  .tk-type { color: var(--syn-type); }
  .tk-meta { color: var(--syn-meta); }
</style>
