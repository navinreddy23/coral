<script lang="ts">
  import { onMount } from 'svelte';

  import type { Frame } from './frame';
  import { backgroundColour, DEFAULT_METRICS, GRAPH_COLUMN_PX, laneColours } from './layout';
  import { drawLanes, resizeCanvas } from './render';

  /**
   * `firstRow` is the row the caller drew at the top of its own list. The canvas shares that
   * origin rather than deriving its own, so a lane node cannot sit a row away from the text it
   * belongs to.
   */
  const { frame, firstRow = 0, height = 400, initials = () => null }: {
    frame: Frame | null;
    firstRow?: number;
    height?: number;
    /** Author initials for a row, or null while its metadata is still loading. */
    initials?: (row: number) => string | null;
  } = $props();

  let canvas: HTMLCanvasElement;
  let colours: string[] = $state([]);
  let background = $state('#ffffff');
  let pending = false;

  onMount(() => {
    colours = laneColours(document.documentElement);
    background = backgroundColour(document.documentElement);
  });

  /**
   * Draws at most once per frame. Drawing straight from a scroll handler fights the compositor
   * and is what makes a canvas feel heavy while scrolling.
   */
  function schedule() {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => {
      pending = false;
      paint();
    });
  }

  function paint() {
    if (!canvas || !frame) return;
    const metrics = DEFAULT_METRICS;
    const width = GRAPH_COLUMN_PX;
    const ctx = resizeCanvas(canvas, width, height, window.devicePixelRatio || 1);
    if (!ctx) return;

    // A screen of rows plus overscan, anchored to the caller's first row.
    const perScreen = Math.ceil(height / metrics.rowHeight);
    const win = {
      first: firstRow,
      last: Math.min(frame.rowCount - 1, firstRow + perScreen + 2),
    };
    drawLanes(ctx, frame, win, metrics, colours, width, height, background, initials);
  }

  $effect(() => {
    // Reading these registers the dependency, so any change repaints.
    void frame;
    void firstRow;
    void initials;
    void height;
    void colours;
    void background;
    schedule();
  });
</script>

<canvas bind:this={canvas} class="lanes" aria-hidden="true"></canvas>

<style>
  .lanes {
    display: block;
    flex: 0 0 auto;
  }
</style>
