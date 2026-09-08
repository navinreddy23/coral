<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icon';
  import type { RefKind } from '../ipc/types';

  const { kind, size = 11 }: { kind: RefKind['kind']; size?: number } = $props();

  /**
   * What sort of ref a label stands for, drawn rather than spelled.
   *
   * The sidebar says "Branches" over its list and can afford a word; a pill eleven pixels
   * tall cannot, and one row can carry a branch, a tag and a stash at once.
   *
   * A remote branch takes the branch shape rather than a cloud: it is a branch, and the row
   * already says which remote it is on.
   */
  const glyph = $derived<IconName>(
    kind === 'tag' ? 'tag' : kind === 'stash' ? 'stash' : 'branch',
  );
</script>

<span class="ref-mark"><Icon name={glyph} {size} /></span>

<style>
  .ref-mark { display: inline-flex; }
</style>
