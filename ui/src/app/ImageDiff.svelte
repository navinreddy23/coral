<script lang="ts" module>
  /** Image types the webview draws, by extension, with the type a blob of each is given. */
  const TYPES: Record<string, string> = {
    png: 'image/png',
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    gif: 'image/gif',
    webp: 'image/webp',
    bmp: 'image/bmp',
    ico: 'image/x-icon',
    avif: 'image/avif',
  };

  function typeOf(file: string): string | null {
    const dot = file.lastIndexOf('.');
    return dot < 0 ? null : (TYPES[file.slice(dot + 1).toLowerCase()] ?? null);
  }

  /** Whether a binary file is one this view can draw. */
  export function isImage(file: string): boolean {
    return typeOf(file) !== null;
  }
</script>

<script lang="ts">
  import { fileBlob } from '../ipc/blob';
  import { messageOf } from '../ipc/error';
  import type { Side } from '../state/diff.svelte';

  const { repo, before, after }: { repo: string; before: Side; after: Side } = $props();

  interface Shown {
    url: string;
    bytes: number;
    width: number | null;
    height: number | null;
  }

  /** Each side once read: an image, null where the side has no such file, or what failed. */
  let sides = $state<{ before: Shown | null; after: Shown | null } | null>(null);
  let error = $state<string | null>(null);

  async function read(side: Side): Promise<Shown | null> {
    const bytes = await fileBlob(repo, side.side, side.file);
    if (bytes === null) return null;
    const type = typeOf(side.file) ?? 'application/octet-stream';
    const url = URL.createObjectURL(new Blob([bytes], { type }));
    return { url, bytes: bytes.byteLength, width: null, height: null };
  }

  $effect(() => {
    const wanted = { repo, before, after };
    let current: typeof sides = null;
    let live = true;
    sides = null;
    error = null;
    void Promise.all([read(wanted.before), read(wanted.after)])
      .then(([b, a]) => {
        current = { before: b, after: a };
        if (live) sides = current;
        else release(current);
      })
      .catch((e: unknown) => {
        if (live) error = messageOf(e);
      });
    // The URLs hold the bytes until they are let go, and a panel stepped through a dozen
    // images would otherwise keep every one of them.
    return () => {
      live = false;
      if (current !== null) release(current);
    };
  });

  function release(held: { before: Shown | null; after: Shown | null }) {
    if (held.before) URL.revokeObjectURL(held.before.url);
    if (held.after) URL.revokeObjectURL(held.after.url);
  }

  function measured(which: 'before' | 'after', img: Element) {
    const s = sides?.[which];
    if (s && img instanceof HTMLImageElement) {
      s.width = img.naturalWidth;
      s.height = img.naturalHeight;
    }
  }

  function size(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

{#if error !== null}
  <p class="error">{error}</p>
{:else if sides === null}
  <p class="muted">Loading the image…</p>
{:else}
  <div class="images">
    {#each [['before', 'Before', sides.before], ['after', 'After', sides.after]] as const as [key, title, shown] (key)}
      <figure class={key}>
        <figcaption>
          <span class="title">{title}</span>
          {#if shown !== null}
            <span class="facts">
              {#if shown.width !== null}{shown.width} × {shown.height} ·{/if}
              {size(shown.bytes)}
            </span>
          {/if}
        </figcaption>
        {#if shown === null}
          <p class="absent">{key === 'before' ? 'Not there before this change' : 'Removed by this change'}</p>
        {:else}
          <div class="canvas">
            <img src={shown.url} alt="{title}: {key === 'before' ? before.file : after.file}" onload={(e) => measured(key, e.currentTarget)} />
          </div>
        {/if}
      </figure>
    {/each}
  </div>
{/if}

<style>
  .images {
    display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-3);
    padding: var(--space-3); min-height: 0;
  }
  figure { margin: 0; display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
  figcaption { display: flex; align-items: baseline; gap: var(--space-2); font-size: var(--text-sm); }
  .title { font-weight: var(--weight-semibold); color: var(--fg-0); }
  .before .title { color: var(--danger); }
  .after .title { color: var(--ok); }
  .facts { color: var(--fg-2); font-variant-numeric: tabular-nums; }
  /* A checkerboard, so a transparent image shows where it is transparent rather than looking
     like a white one. Two tokens, so it follows the theme. */
  .canvas {
    display: flex; align-items: center; justify-content: center; padding: var(--space-3);
    border: var(--hairline) solid var(--border); border-radius: var(--radius-2);
    background-color: var(--bg-0);
    background-image:
      linear-gradient(45deg, var(--bg-2) 25%, transparent 25%, transparent 75%, var(--bg-2) 75%),
      linear-gradient(45deg, var(--bg-2) 25%, transparent 25%, transparent 75%, var(--bg-2) 75%);
    background-size: 16px 16px;
    background-position: 0 0, 8px 8px;
  }
  img { max-width: 100%; max-height: 60vh; object-fit: contain; image-rendering: auto; }
  .absent {
    margin: 0; padding: var(--space-4); text-align: center; color: var(--fg-2);
    border: var(--hairline) dashed var(--border); border-radius: var(--radius-2);
  }
  .muted { color: var(--fg-2); padding: var(--space-3); margin: 0; }
  .error { color: var(--danger); padding: var(--space-3); margin: 0; }
</style>
