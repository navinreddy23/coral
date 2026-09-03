<script lang="ts">
  /**
   * A drag handle between two panes.
   *
   * Reports the width it would like rather than setting it, so the owner stays the single
   * place a width is clamped and persisted.
   */
  const {
    value,
    min,
    max,
    label,
    grows = 'right',
    onresize,
    onreset,
  }: {
    value: number;
    min: number;
    max: number;
    label: string;
    /** Which way the pane being sized grows when the handle moves right. */
    grows?: 'right' | 'left';
    onresize: (px: number) => void;
    onreset?: () => void;
  } = $props();

  let dragging = $state(false);
  let startX = 0;
  let startValue = 0;

  // -1 for a pane anchored to the right edge: dragging the handle left makes it wider.
  const sign = $derived(grows === 'left' ? -1 : 1);

  function down(event: PointerEvent & { currentTarget: HTMLDivElement }) {
    startX = event.clientX;
    startValue = value;
    dragging = true;
    // Capture, or the drag stops the moment the pointer outruns a 9px-wide target.
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function move(event: PointerEvent) {
    if (!dragging) return;
    onresize(startValue + (event.clientX - startX) * sign);
  }

  function up(event: PointerEvent & { currentTarget: HTMLDivElement }) {
    dragging = false;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function key(event: KeyboardEvent) {
    const step = event.shiftKey ? 32 : 8;
    if (event.key === 'ArrowLeft') onresize(value - step * sign);
    else if (event.key === 'ArrowRight') onresize(value + step * sign);
    else return;
    event.preventDefault();
  }
</script>

<!--
  A focusable separator, which is the ARIA window-splitter pattern: it carries its current
  width so a screen reader can announce the drag, and the arrow keys move it without a pointer.

  The rule below treats every separator as non-interactive. That holds for the decorative kind,
  not for a window splitter, which is focusable and operable by definition; there is no other
  role that means "resizes the pane beside me".
-->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuenow={Math.round(value)}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
  ondblclick={onreset}
></div>

<style>
  .splitter {
    flex: 0 0 auto; width: 9px; margin: 0 -4px; z-index: 3;
    cursor: col-resize; background: none; border: 0; padding: 0;
    /* The line itself is one pixel; the rest of the width is target area, which is why the
       handle is pulled back over its neighbours with a negative margin. */
    background-image: linear-gradient(to right, transparent 4px, var(--border) 4px 5px, transparent 5px);
  }
  .splitter:hover, .splitter.dragging, .splitter:focus-visible {
    background-image: linear-gradient(to right, transparent 3px, var(--accent) 3px 6px, transparent 6px);
    outline: none;
  }
</style>
