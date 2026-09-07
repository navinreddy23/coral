<script lang="ts">
  import type { Profile } from '../ipc/profiles';
  import { laneColour } from './lane';

  /**
   * Which profile the window is in, at the leading edge of the title strip.
   *
   * A chip rather than a menu bar entry, because the answer has to be readable without being
   * asked for: every tab, every recent repository and the identity a clone will carry come
   * from it, and somebody who has forgotten which one they are in is looking at the wrong
   * repositories with no sign that anything is wrong.
   *
   * The menu it opens is built by the window, in `App.svelte`, like every other menu here —
   * which is also what keeps it out of the way of the strip's own drag handling.
   */
  const { profile, onMenu }: {
    profile: Profile;
    onMenu: (event: MouseEvent) => void;
  } = $props();
</script>

<button
  class="chip"
  style:--band={laneColour(profile.colour)}
  onclick={onMenu}
  oncontextmenu={onMenu}
  title="Profile: {profile.name}. Its tabs, its recent repositories and the identity a clone will carry."
  aria-label="Profile: {profile.name}"
>
  <span class="dot" aria-hidden="true"></span>{profile.name}
</button>

<style>
  .chip {
    flex: 0 0 auto; max-width: 11em;
    display: inline-flex; align-items: center; gap: 5px;
    font: inherit; font-size: 11px; font-weight: 600; cursor: pointer;
    padding: 2px var(--space-2); border-radius: 999px;
    border: 1px solid var(--border); background: var(--bg-1); color: var(--fg-1);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .chip:hover { background: var(--bg-3); color: var(--fg-0); }
  /* The colour is what makes two profiles tell apart at a glance; the name is what says which.
     Both, because eight lane colours run out before names do. */
  .dot {
    flex: 0 0 auto; width: 8px; height: 8px; border-radius: 50%; background: var(--band);
  }
</style>
