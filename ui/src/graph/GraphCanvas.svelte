<script lang="ts">
  import { onMount } from 'svelte';

  import type { Frame } from './frame';
  import { DEFAULT_METRICS, GRAPH_COLUMN_PX, laneColours, visibleRows } from './layout';
  import { drawLanes, resizeCanvas } from './render';

  const { frame, scrollTop = 0, height = 400 }: {
    frame: Frame | null;
    scrollTop?: number;
    height?: number;
  } = $props();

  let canvas: HTMLCanvasElement;
  let colours: string[] = $state([]);
  let pending = false;

  onMount(() => {
    colours = laneColours(document.documentElement);
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

    const win = visibleRows(scrollTop, height, frame.totalRows, metrics);
    drawLanes(ctx, frame, win, metrics, colours, width, height);
  }

  $effect(() => {
    // Reading these registers the dependency, so any change repaints.
    void frame;
    void scrollTop;
    void height;
    void colours;
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
