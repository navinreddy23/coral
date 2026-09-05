<script lang="ts" module>
  /** Which host a remote URL belongs to, as far as a glyph is concerned. */
  export type HostMarkKind = 'github' | 'gitlab' | 'other';

  /**
   * Recognises a host from a remote URL.
   *
   * Deliberately looser than `coral-hosting`'s own detection, which has to be sure enough to
   * call an API. This only chooses a picture, so `github.` and `gitlab.` anywhere in the host
   * is enough, and everything else gets the cloud rather than a guess.
   */
  export function hostOf(url: string): HostMarkKind {
    const host = url.replace(/^[a-z+]+:\/\//iu, '').replace(/^[^@]*@/u, '').split(/[/:]/u)[0] ?? '';
    const lower = host.toLowerCase();
    if (lower === 'github.com' || lower.startsWith('github.')) return 'github';
    if (lower === 'gitlab.com' || lower.startsWith('gitlab.')) return 'gitlab';
    return 'other';
  }
</script>

<script lang="ts">
  const { kind, title, size = 12 }: { kind: HostMarkKind; title?: string; size?: number } = $props();
</script>

<!--
  Drawn rather than fetched. Every logo here is a path in the page: an <img> would need the mark
  on disk and a request for it, and the webview is served from a custom protocol where that is
  one more thing to get wrong for a 14-pixel glyph.

  `currentColor` throughout, so the mark takes the colour of whatever row it sits on and works
  in both themes without a second copy.
-->
<svg
  class="mark {kind}"
  viewBox="0 0 16 16"
  width={size}
  height={size}
  aria-hidden={title === undefined}
  role={title === undefined ? 'presentation' : 'img'}
  fill="currentColor"
>
  {#if title}<title>{title}</title>{/if}
  {#if kind === 'github'}
    <path
      d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38
         0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13
         -.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66
         .07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15
         -.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27
         .68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12
         .51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48
         0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z"
    />
  {:else if kind === 'gitlab'}
    <!-- The tanuki's outline. At twelve pixels the four-part construction reads as noise, so
         this is the silhouette in one colour. -->
    <path
      d="M15.73 6.1 15.71 6.05 13.55 0.4a.56.56 0 0 0-.22-.27.58.58 0 0 0-.67.04.58.58 0 0
         0-.19.29l-1.46 4.47H5l-1.46-4.47a.57.57 0 0 0-.19-.29.58.58 0 0 0-.67-.04.57.57 0 0
         0-.22.27L.29 6.04l-.02.05a4.01 4.01 0 0 0 1.33 4.64l.01.01.02.01 3.29 2.47 1.63 1.23
         .99.75a.67.67 0 0 0 .81 0l.99-.75 1.63-1.23 3.31-2.48.01-.01a4.01 4.01 0 0 0
         1.45-4.63Z"
    />
  {:else}
    <!-- A cloud, for a host Coral has no picture of. Not a question mark: the remote works
         perfectly well, it is simply not one of the two we can name. -->
    <path
      d="M12.3 6.6a4.2 4.2 0 0 0-8.05-.7A3.2 3.2 0 0 0 4.4 12.3h7.6a2.85 2.85 0 0 0 .3-5.7Z"
    />
  {/if}
</svg>

<style>
  .mark { flex: 0 0 auto; vertical-align: -1px; }
  /* The cloud is a background object rather than a brand, so it sits back a little. */
  .mark.other { opacity: 0.7; }
</style>
