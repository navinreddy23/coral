<script lang="ts">
  import { startResize, type ResizeDirection } from '../ipc/window';

  const { active }: {
    /** False while the desktop is drawing the window's border itself. */
    active: boolean;
  } = $props();

  /**
   * The eight places a window can be pulled from, in the order they are drawn: the four edges
   * first, then the four corners over them, so a corner wins where the two overlap.
   */
  const grips: { at: ResizeDirection; label: string }[] = [
    { at: 'North', label: 'top edge' },
    { at: 'South', label: 'bottom edge' },
    { at: 'West', label: 'left edge' },
    { at: 'East', label: 'right edge' },
    { at: 'NorthWest', label: 'top left corner' },
    { at: 'NorthEast', label: 'top right corner' },
    { at: 'SouthWest', label: 'bottom left corner' },
    { at: 'SouthEast', label: 'bottom right corner' },
  ];

  function pull(event: MouseEvent, at: ResizeDirection) {
    // Only the primary button, and only a press: the window manager owns the pointer from here
    // and never sends the matching release back to the page.
    if (event.button !== 0) return;
    event.preventDefault();
    void startResize(at);
  }
</script>

<!--
  The border the desktop no longer draws.

  An undecorated GTK window has no resize handles of its own, so the four pixels along each
  edge are given back here. They are the width a decorated window's invisible border is, which
  is what a hand reaching for an edge expects to find.
-->
{#if active}
  {#each grips as grip (grip.at)}
    <div
      class="grip {grip.at.toLowerCase()}"
      role="presentation"
      aria-label="Resize from the {grip.label}"
      onmousedown={(e) => pull(e, grip.at)}
    ></div>
  {/each}
{/if}

<style>
  .grip { position: fixed; z-index: 90; }

  .north { top: 0; left: 0; right: 0; height: 4px; cursor: n-resize; }
  .south { bottom: 0; left: 0; right: 0; height: 4px; cursor: s-resize; }
  .west { top: 0; bottom: 0; left: 0; width: 4px; cursor: w-resize; }
  .east { top: 0; bottom: 0; right: 0; width: 4px; cursor: e-resize; }

  /* Over the edges, so the diagonal wins in the twelve pixels where they meet. */
  .northwest, .northeast, .southwest, .southeast {
    width: 12px; height: 12px; z-index: 91;
  }
  .northwest { top: 0; left: 0; cursor: nw-resize; }
  .northeast { top: 0; right: 0; cursor: ne-resize; }
  .southwest { bottom: 0; left: 0; cursor: sw-resize; }
  .southeast { bottom: 0; right: 0; cursor: se-resize; }
</style>
