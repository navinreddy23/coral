<script lang="ts">
  import Icon from './Icon.svelte';
  import type { LfsLock } from '../ipc/types';

  /** A file's lock on the LFS server, marked yours or somebody else's; nothing when unlocked. */
  const { lock }: { lock: LfsLock | undefined } = $props();
</script>

{#if lock}
  <span
    class="lock"
    class:theirs={!lock.ours}
    title={lock.ours ? 'Locked by you' : `Locked by ${lock.owner || 'somebody else'}`}
  >
    <Icon name="lock" size={11} />
  </span>
{/if}

<style>
  .lock { flex: 0 0 auto; display: inline-flex; margin-left: auto; color: var(--accent); }
  .lock.theirs { color: var(--warn); }
</style>
