<script lang="ts">
  import type { Misspelling } from '../ipc/types';
  import { COPIED, runs } from './underline';

  /**
   * The misspelt words of a text field, underlined in place.
   *
   * A text box cannot style part of its own value, so the text is drawn again on a layer laid
   * over the field, in the field's own type and spacing, and only the underlines of that copy
   * show: its letters are transparent. Over the field rather than under it, because the field
   * paints its own background — WebKit sharpens text only over a backdrop it knows is opaque —
   * and the copy has no text of its own to blur.
   */
  const {
    field,
    text,
    marks,
  }: {
    field: HTMLInputElement | HTMLTextAreaElement | null;
    text: string;
    marks: Misspelling[];
  } = $props();

  let layer = $state<HTMLDivElement | null>(null);
  let frame = $state('');
  let padding = $state({ top: 0, right: 0, bottom: 0, left: 0 });
  /** The field's inside, padding included, which is what its text scrolls within. */
  let inside = $state({ width: 0, height: 0 });
  let scroll = $state({ left: 0, top: 0 });

  /** An input is one line, centred, that scrolls sideways and is cut off at its padding. */
  const single = $derived(field instanceof HTMLInputElement);
  const pieces = $derived(runs(text, marks));
  const content = $derived({
    width: Math.max(0, inside.width - padding.left - padding.right),
    height: Math.max(0, inside.height - padding.top - padding.bottom),
  });

  function measure(): void {
    if (!field) return;
    const style = getComputedStyle(field);
    frame = COPIED.map((name) => `${name}:${style.getPropertyValue(name)}`).join(';');
    const px = (name: string) => Number.parseFloat(style.getPropertyValue(name)) || 0;
    padding = {
      top: px('padding-top'),
      right: px('padding-right'),
      bottom: px('padding-bottom'),
      left: px('padding-left'),
    };
    // `clientWidth` leaves out a scrollbar that takes room, which is the width the text wraps in.
    inside = { width: field.clientWidth, height: field.clientHeight };
    follow();
  }

  function follow(): void {
    if (field) scroll = { left: field.scrollLeft, top: field.scrollTop };
  }

  /*
   * The field scrolls on its own, and not always with an event to say so: WebKit moves an
   * input back to its start when it loses focus, and a caret moved past the edge drags the
   * text after it. So the position is read again on anything that can move it, and once more
   * a frame later, when the move has happened.
   */
  $effect(() => {
    const el = field;
    if (!el) return;
    const moves = ['scroll', 'input', 'keydown', 'keyup', 'select', 'pointerdown', 'pointerup', 'focus', 'blur'];
    const sync = () => {
      follow();
      requestAnimationFrame(follow);
    };
    for (const name of moves) el.addEventListener(name, sync);
    const resized = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    resized?.observe(el);
    measure();
    return () => {
      for (const name of moves) el.removeEventListener(name, sync);
      resized?.disconnect();
    };
  });

  // Marks arriving put the layer on screen, and the text it copies may have scrolled meanwhile.
  $effect(() => {
    void pieces;
    follow();
  });

  /** The misspelling drawn under a point, if there is one there to see. */
  export function markAt(x: number, y: number): Misspelling | null {
    const clip = layer?.querySelector('.clip')?.getBoundingClientRect();
    if (!layer || !clip || x < clip.left || x > clip.right || y < clip.top || y > clip.bottom) {
      return null;
    }
    for (const span of layer.querySelectorAll<HTMLElement>('.miss')) {
      for (const r of span.getClientRects()) {
        if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
          return marks.find((m) => m.start === Number(span.dataset['start'])) ?? null;
        }
      }
    }
    return null;
  }

  /** Where a menu about `mark` opens from the keyboard: under the word's first letter. */
  export function placeOf(mark: Misspelling): { x: number; y: number } | null {
    const span = layer?.querySelector<HTMLElement>(`.miss[data-start="${mark.start}"]`);
    const first = span?.getClientRects()[0];
    return first ? { x: first.left, y: first.bottom } : null;
  }
</script>

{#if marks.length > 0}
  <div class="misspellings" class:single bind:this={layer} style={frame} aria-hidden="true">
    <div
      class="clip"
      style:left="{single ? padding.left : 0}px"
      style:top="{single ? padding.top : 0}px"
      style:width="{single ? content.width : inside.width}px"
      style:height="{single ? content.height : inside.height}px"
    >
      <div
        class="text"
        style:padding={single ? '0' : `${padding.top}px ${padding.right}px ${padding.bottom}px ${padding.left}px`}
        style:width={single ? null : `${content.width}px`}
        style:line-height={single ? `${content.height}px` : null}
        style:transform="translate({-scroll.left}px, {-scroll.top}px)"
      >{#each pieces as piece (piece.start)}{#if piece.marked}<span class="miss" data-start={piece.start}>{piece.text}</span>{:else}{piece.text}{/if}{/each}</div>
    </div>
  </div>
{/if}

<style>
  /* Raised, because WebKitGTK gives a focused text field a compositing layer of its own, and
     that layer covered the underlines of whichever field had the caret. */
  .misspellings {
    position: absolute; inset: 0; z-index: 1; pointer-events: none; user-select: none;
    border-style: solid; border-color: transparent; background: none; color: transparent;
    overflow: hidden;
  }
  .clip { position: absolute; overflow: hidden; }
  .text {
    position: absolute; left: 0; top: 0; box-sizing: content-box;
    white-space: pre-wrap; overflow-wrap: break-word;
  }
  .single .text { white-space: pre; }
  .miss {
    text-decoration-line: underline; text-decoration-style: wavy;
    text-decoration-color: var(--danger); text-decoration-skip-ink: none;
  }
</style>
