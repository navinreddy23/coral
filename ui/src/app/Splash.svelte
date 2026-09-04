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
    { id: 'opening', label: 'Reading the repository' },
    { id: 'walking', label: 'Walking the commit graph' },
    { id: 'ordering', label: 'Putting the commits in topological order' },
  ] as const;

  const reached = $derived(steps.findIndex((s) => s.id === stage));
</script>

<div class="splash">
  <div class="badge" aria-hidden="true">
    <span class="ring"></span>
    <span class="mark">C</span>
  </div>

  <h2>{repo}</h2>
  <p class="path mono">{path}</p>

  <ol class="steps">
    {#each steps as step, i (step.id)}
      <li class:done={i < reached} class:now={i === reached}>
        <span class="dot" aria-hidden="true"></span>
        {step.label}
      </li>
    {/each}
  </ol>

  <p class="note">
    The first screen appears before the walk finishes, so the graph may reorder itself once.
  </p>
</div>

<style>
  .splash {
    flex: 1; display: flex; flex-direction: column;
    justify-content: center; align-items: center; gap: var(--space-2);
    padding: var(--space-5); text-align: center; background: var(--bg-0);
  }
  /*
   * A spinning ring rather than a bar: nothing here knows how far along it is, and a bar that
   * cannot fill honestly is worse than one that does not claim to.
   */
  .badge { position: relative; width: 52px; height: 52px; margin-bottom: var(--space-2); }
  .ring {
    position: absolute; inset: 0; border-radius: 50%;
    border: 3px solid var(--bg-3); border-top-color: var(--accent);
    animation: spin 900ms linear infinite;
  }
  .mark {
    position: absolute; inset: 0; display: grid; place-items: center;
    font-size: 20px; font-weight: 700; color: var(--accent);
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  /* Respecting the system setting: a ring that never stops is exactly what this asks about. */
  @media (prefers-reduced-motion: reduce) {
    .ring { animation: none; border-top-color: var(--accent); }
  }

  h2 { margin: 0; font-size: 15px; font-weight: 600; color: var(--fg-0); }
  .path {
    margin: 0; font-size: 11px; color: var(--fg-2);
    max-width: 40em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  .steps {
    list-style: none; margin: var(--space-4) 0 0; padding: 0;
    display: flex; flex-direction: column; gap: var(--space-2);
    text-align: left; font-size: 12px; color: var(--fg-2);
  }
  .steps li { display: flex; align-items: center; gap: var(--space-2); }
  .steps li.done { color: var(--fg-1); }
  .steps li.now { color: var(--fg-0); font-weight: 600; }
  .dot {
    flex: 0 0 auto; width: 8px; height: 8px; border-radius: 50%;
    background: var(--bg-3);
  }
  .steps li.done .dot { background: var(--ok); }
  .steps li.now .dot { background: var(--accent); }

  .note {
    margin: var(--space-4) 0 0; max-width: 34em; line-height: 1.5;
    font-size: 11px; color: var(--fg-2);
  }
</style>
