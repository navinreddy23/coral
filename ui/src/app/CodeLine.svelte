<script lang="ts">
  import type { Span } from '../diff/word';

  /**
   * One line of a diff, with the words that changed inside it marked.
   *
   * A component rather than a block in the panel because the markup has to carry no whitespace
   * of its own: the text is rendered under `white-space: pre`, so a newline between two tags
   * would show up as a space in the middle of the code.
   */
  const { text, spans }: {
    text: string;
    /** Null for a line with nothing to mark, which is most of them. */
    spans: Span[] | null;
  } = $props();
</script>

{#if spans === null}{text}{:else}{#each spans as span, i (i)}{#if span.marked}<mark>{span.text}</mark>{:else}{span.text}{/if}{/each}{/if}

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
    padding: 0; border-radius: 2px;
  }
</style>
