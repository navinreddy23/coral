<script lang="ts">
  const { repo, path, stage }: {
    /** The repository's own name, which is what the user asked for. */
    repo: string;
    path: string;
    /** Which step is running, so the wait is accountable rather than blank. */
    stage: 'opening' | 'walking' | 'ordering';
  } = $props();

  /**
   * The three steps, in order. A large repository spends seconds in the middle one, and a
   * progress bar that cannot say how far along it is says less than a named step does.
   */
  const steps = [
    { id: 'opening', label: 'Reading', says: 'Reading the repository' },
    { id: 'walking', label: 'Walking', says: 'Walking the commit graph' },
    { id: 'ordering', label: 'Ordering', says: 'Putting the commits in topological order' },
  ] as const;

  const reached = $derived(steps.findIndex((s) => s.id === stage));
  const saying = $derived(steps[reached]?.says ?? steps[0].says);
</script>

<!--
  A commit graph drawing itself, which is the thing being waited for.

  A spinner says only that something is happening. This says what: lanes grow, branch, and
  merge back, and a pulse runs down the trunk for as long as the walk does. It is the same
  shape, the same eight-colour palette and the same geometry the real graph is drawn with, so
  the wait looks like the beginning of the thing rather than an interruption before it.
-->
<div class="splash">
  <svg
    class="art"
    viewBox="0 0 200 132"
    width="200"
    height="132"
    fill="none"
    stroke-width="2.5"
    stroke-linecap="round"
    aria-hidden="true"
  >
    <!-- Every path is normalised to a length of one, so one dash animation draws all of them
         however long each actually is. -->
    <path class="lane trunk" pathLength="1" style="--d: 0ms" d="M36 6 V126" />
    <path class="lane branch-a" pathLength="1" style="--d: 260ms"
      d="M36 28 C36 40 76 34 76 46 V84 C76 96 36 90 36 102" />
    <path class="lane branch-b" pathLength="1" style="--d: 480ms"
      d="M36 56 C36 68 116 62 116 74 V122" />

    <!-- The pulse: a short dash on a copy of the trunk, travelling for as long as this is up. -->
    <path class="pulse" pathLength="1" d="M36 6 V126" />

    <g class="nodes">
      <circle class="n trunk" cx="36" cy="6" r="5" style="--d: 220ms" />
      <circle class="n trunk" cx="36" cy="28" r="5.5" style="--d: 320ms" />
      <circle class="n trunk" cx="36" cy="56" r="5.5" style="--d: 420ms" />
      <circle class="n trunk" cx="36" cy="102" r="6" style="--d: 900ms" />
      <circle class="n trunk" cx="36" cy="126" r="5" style="--d: 1000ms" />
      <circle class="n branch-a" cx="76" cy="46" r="5" style="--d: 560ms" />
      <circle class="n branch-a" cx="76" cy="66" r="5" style="--d: 660ms" />
      <circle class="n branch-a" cx="76" cy="84" r="5" style="--d: 760ms" />
      <circle class="n branch-b" cx="116" cy="74" r="5" style="--d: 700ms" />
      <circle class="n branch-b" cx="116" cy="100" r="5" style="--d: 820ms" />
    </g>
  </svg>

  <h2>{repo}</h2>
  <p class="path mono">{path}</p>

  <ol class="rail">
    {#each steps as step, i (step.id)}
      <li class:done={i < reached} class:now={i === reached}>
        <span class="node" aria-hidden="true"></span>
        <span class="label">{step.label}</span>
      </li>
    {/each}
  </ol>

  <p class="says" aria-live="polite">{saying}</p>

  <p class="note">
    The first screen appears before the walk finishes, so the graph may reorder itself once.
  </p>
</div>

<style>
  .splash {
    flex: 1; display: flex; flex-direction: column;
    justify-content: center; align-items: center;
    padding: var(--space-5); text-align: center; background: var(--bg-0);
  }

  /* ---- the graph ---- */

  /* Drawn at 200x132 and shown larger, so the strokes keep the weight the real graph uses
     while the whole thing carries the screen. */
  .art { width: 250px; height: 165px; margin-bottom: var(--space-4); overflow: visible; }

  /*
   * The dash lives in the keyframes, not on the element, so a lane that is never animated is
   * a drawn lane rather than an invisible one. Written the other way round the resting state
   * is "hidden", and anything that stops the animation — a delay that fails to resolve, a
   * pruned keyframe, a setting — leaves an empty box where the graph should be.
   */
  .lane { animation: draw 820ms ease-out var(--d) both; }
  .trunk { stroke: var(--lane-1); }
  .branch-a { stroke: var(--lane-2); }
  .branch-b { stroke: var(--lane-4); }

  @keyframes draw {
    from { stroke-dasharray: 1; stroke-dashoffset: 1; }
    to { stroke-dasharray: 1; stroke-dashoffset: 0; }
  }

  /*
   * The pulse waits for the trunk to finish drawing, then runs it forever. A twelfth of the
   * path is lit at a time, which at this size is about a commit and a half — short enough to
   * read as travelling rather than as a second lane.
   */
  .pulse {
    stroke: var(--accent); stroke-width: 3;
    stroke-dasharray: 0.08 0.92; stroke-dashoffset: 0.08;
    opacity: 0; animation: run 1900ms linear 900ms infinite;
  }

  @keyframes run {
    0% { stroke-dashoffset: 0.08; opacity: 0; }
    12% { opacity: 0.9; }
    78% { opacity: 0.9; }
    100% { stroke-dashoffset: -0.92; opacity: 0; }
  }

  /* Drawn from their own centre, so the overshoot is a node arriving rather than a node
     sliding in from the corner of the drawing. */
  .n {
    transform-box: fill-box; transform-origin: center;
    stroke-width: 2.5; fill: var(--bg-0);
    animation: pop 420ms cubic-bezier(0.34, 1.56, 0.64, 1) var(--d) both;
  }
  .n.trunk { stroke: var(--lane-1); }
  .n.branch-a { stroke: var(--lane-2); }
  .n.branch-b { stroke: var(--lane-4); }

  @keyframes pop {
    from { transform: scale(0); }
    to { transform: scale(1); }
  }

  /* ---- what is being opened ---- */

  h2 { margin: 0; font-size: var(--text-xl); font-weight: 600; letter-spacing: -0.01em; color: var(--fg-0); }
  .path {
    margin: 4px 0 0; font-size: var(--text-sm); color: var(--fg-2);
    max-width: 40em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  /* ---- the three steps, as a track rather than a list ---- */

  .rail {
    list-style: none; margin: var(--space-5) 0 0; padding: 0;
    display: flex; width: min(26rem, 100%);
  }
  .rail li {
    flex: 1; position: relative;
    display: flex; flex-direction: column; align-items: center; gap: var(--space-2);
  }

  /* The line into a station, from the one before it. */
  .rail li + li::before {
    content: ''; position: absolute; top: 5px; left: -50%; right: 50%; height: 2px;
    border-radius: 1px; background: var(--bg-3);
  }
  .rail li.done + li::before { background: var(--accent); }

  /* And a lit stretch running along the one still being covered. */
  .rail li.now + li::after {
    content: ''; position: absolute; top: 5px; left: -50%; width: 30%; height: 2px;
    border-radius: 1px; background: var(--accent);
    animation: travel 1500ms ease-in-out infinite;
  }
  @keyframes travel {
    0% { transform: translateX(0); opacity: 0; }
    15% { opacity: 1; }
    85% { opacity: 1; }
    100% { transform: translateX(233%); opacity: 0; }
  }

  .node {
    position: relative; width: 10px; height: 10px; border-radius: 50%;
    background: var(--bg-3);
  }
  .rail li.done .node { background: var(--accent); }
  .rail li.now .node { background: var(--accent); }

  /* A ring leaving the station that is working, which is the one thing on the track that has
     to be findable without reading the labels. */
  .rail li.now .node::after {
    content: ''; position: absolute; inset: -5px; border-radius: 50%;
    border: 2px solid var(--accent);
    animation: halo 1700ms ease-out infinite;
  }
  @keyframes halo {
    0% { transform: scale(0.55); opacity: 0.7; }
    100% { transform: scale(1.45); opacity: 0; }
  }

  .label {
    font-size: var(--text-sm); letter-spacing: 0.04em; text-transform: uppercase; color: var(--fg-2);
  }
  .rail li.done .label { color: var(--fg-1); }
  .rail li.now .label { color: var(--fg-0); font-weight: 600; }

  .says { margin: var(--space-3) 0 0; font-size: var(--text-md); color: var(--fg-1); }

  .note {
    margin: var(--space-5) 0 0; max-width: 32em; line-height: 1.5;
    font-size: var(--text-sm); color: var(--fg-2);
  }

  /*
   * The system setting is exactly about this screen: something that moves for as long as a
   * wait lasts. Everything arrives in its finished state instead, and the track still says
   * which step is running.
   */
  @media (prefers-reduced-motion: reduce) {
    .lane { animation: none; }
    .n { animation: none; }
    .pulse { display: none; }
    .rail li.now + li::after { display: none; }
    .rail li.now .node::after { animation: none; opacity: 0.5; transform: scale(1.2); }
  }
</style>
