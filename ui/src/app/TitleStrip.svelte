<script lang="ts">
  import Icon from './Icon.svelte';
  import Mark from './Mark.svelte';
  import ProfileChip from './ProfileChip.svelte';
  import TabBar from './TabBar.svelte';
  import type { TabsState } from '../state/tabs.svelte';
  import type { Profile } from '../ipc/profiles';

  /**
   * The window's own top line: the mark, the name, the profile, the tabs, and the three
   * buttons that act on the window rather than on the repository.
   *
   * It is the title bar. Coral asks the desktop not to draw one, which buys back a whole row
   * on a laptop screen and is why the window controls are here — and why a window manager
   * that handles an undecorated window badly can put the desktop's own bar back, which is what
   * `decorated` is.
   */
  const {
    tabs,
    newTab,
    profile,
    provisional,
    dark,
    themeHint,
    decorated,
    maximised,
    onDrag,
    onMenu,
    onOpenTab,
    onCloseNew,
    onPickTab,
    onAsk,
    onProfileMenu,
    onPreferences,
    onToggleTheme,
    onMinimise,
    onMaximise,
    onClose,
  }: {
    tabs: TabsState;
    /** True while the start page is showing, which is a tab as far as the strip is concerned. */
    newTab: boolean;
    profile: Profile;
    /** True while the graph is in commit-time order, waiting for the topological walk. */
    provisional: boolean;
    dark: boolean;
    /** What the theme button's tooltip says, which depends on what it is following. */
    themeHint: string;
    /** True when the desktop draws its own title bar, in which case this one draws no buttons. */
    decorated: boolean;
    maximised: boolean;
    /** Dragging the strip moves the window; right-clicking it opens the desktop's menu. */
    onDrag: (event: MouseEvent) => void;
    onMenu: (event: MouseEvent) => void;
    onOpenTab: () => void;
    onCloseNew: () => void;
    onPickTab: () => void;
    onAsk: (title: string, detail: string, initial: string) => Promise<string | null>;
    onProfileMenu: (event: MouseEvent) => void;
    onPreferences: () => void;
    onToggleTheme: () => void;
    onMinimise: () => void;
    onMaximise: () => void;
    onClose: () => void;
  } = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<header class="strip" onmousedown={onDrag} oncontextmenu={onMenu}>
  <!-- The application's own mark, the same one the icon carries: one commit and the two
       branches that leave it. -->
  <span class="logo sit"><Mark size={18} /></span>
  <!-- The window has no title bar to carry the name any more, so the strip does. -->
  <h1 class="sit">Coral</h1>

  <!-- Before the tabs, because it is what they belong to: these are this profile's tabs and
       no other's, and somebody who has forgotten which profile they are in is looking at the
       wrong repositories with nothing on screen saying so. -->
  <span class="sit"><ProfileChip {profile} onMenu={onProfileMenu} /></span>

  <TabBar
    {tabs}
    {newTab}
    onOpen={onOpenTab}
    {onCloseNew}
    onPick={onPickTab}
    {onAsk}
  />

  {#if provisional}
    <span class="chip warn sit" title="Commit-time order, being replaced by the topological walk">
      provisional order
    </span>
  {/if}

  <!-- The activity log is not here: its way in is the corner of the status bar, beside the
       work it reports on rather than beside the window's own buttons. -->
  <div class="tools sit">
    <button
      class="chrome"
      onclick={onPreferences}
      title="SSH keys, signing and preferences"
      aria-label="Settings"
    ><Icon name="settings" /></button>
    <!--
      Both faces are drawn and one is turned away, rather than swapped in and out. A theme
      switch that changes under the pointer with no movement reads as a redraw; turning is
      the one thing that says the button did something.
    -->
    <button
      class="chrome swap"
      class:dark
      onclick={onToggleTheme}
      title={themeHint}
      aria-label="Switch theme"
    >
      <span class="face moon"><Icon name="moon" /></span>
      <span class="face sun"><Icon name="sun" /></span>
    </button>
  </div>

  {#if !decorated}
    <span class="split sit" aria-hidden="true"></span>
    <!--
      Full height and hard against the corner, the way a title bar's buttons have always
      been. A rounded 26 pixel button set eight pixels in is a comfortable target in the
      middle of a window and a poor one here, where the whole point of the corner is that
      the pointer cannot overshoot it.
    -->
    <div class="sysbar">
      <button class="sys" onclick={onMinimise} title="Minimise" aria-label="Minimise">
        <Icon name="minimise" size={14} />
      </button>
      <button
        class="sys"
        onclick={onMaximise}
        title={maximised ? 'Restore' : 'Maximise'}
        aria-label={maximised ? 'Restore' : 'Maximise'}
      >
        <Icon name={maximised ? 'restore' : 'maximise'} size={14} />
      </button>
      <button class="sys shut" onclick={onClose} title="Close" aria-label="Close">
        <Icon name="close" size={14} />
      </button>
    </div>
  {/if}
</header>

<style>
  /* The one place in the chrome that wears the brand colour rather than the accent: it is
     the mark, not a control. */
  .logo { flex: 0 0 auto; display: flex; color: var(--brand); }
  /*
   * The title bar, the tab strip and the window's own buttons, on one line.
   *
   * Its height is the tab's: the strip is what the tabs stand on and nothing else in it is
   * allowed to make the window taller.
   */
  .strip {
    display: flex; align-items: center; gap: var(--space-2);
    height: var(--strip); box-sizing: border-box; padding: 0 var(--space-2) 0 var(--space-3);
    background: var(--bg-2); box-shadow: inset 0 -1px 0 var(--border);
  }
  /*
   * Everything that is not a tab, on the tabs' centre line.
   *
   * The tabs are 32 pixels tall against the bottom edge of a 40 pixel strip, so their middle
   * is four pixels below the strip's. A centred item with eight pixels of space above it lands
   * there whatever its own height is, which is why this is one rule and not one per item.
   */
  .sit { align-self: center; margin-top: var(--space-2); }
  /* The wordmark is not a control, so it does not wear the accent. The mark beside it carries
     the colour; the name is set in the page's own strongest text, which is what a name is. */
  h1 {
    font-size: var(--text-md); font-weight: 600; margin: 0; color: var(--fg-0);
    letter-spacing: -0.01em; flex: 0 0 auto; background: var(--bg-1);
  }
  .tools { display: flex; align-items: center; gap: 2px; margin-left: auto; }
  /* The window's own three, set apart from Coral's three: one set acts on what the window is
     showing, the other on the window. */
  .split { width: 1px; height: 16px; background: var(--border-strong); opacity: 0.6; }
  /* Pulled out over the strip's own padding, so the last button ends at the window's edge. */
  .sysbar {
    display: flex; align-self: stretch;
    margin-left: var(--space-2); margin-right: calc(-1 * var(--space-2));
  }
  .sys {
    display: flex; align-items: center; justify-content: center;
    /* Full height so the target runs to the top edge, with the glyph pushed down onto the
       same line as everything else in the strip. */
    width: 44px; padding: var(--space-2) 0 0; cursor: pointer;
    border: 0; background: transparent; color: var(--fg-1);
  }
  .sys:hover { background: var(--bg-3); color: var(--fg-0); }
  /* The one button that cannot be undone, which is the one convention has painted red since
     windows had buttons. */
  /* The page's own colour, not white: the close button turns the danger red, which is a
     dark fill in the light theme and a light one in the dark theme, so what reads on it is
     whatever the page is not. `--accent-fg` would be the wrong token — this is not an
     accent. */
  .sys.shut:hover { background: var(--danger); color: var(--bg-0); }
  .chip {
    font-size: var(--text-sm); padding: 1px var(--space-2); border-radius: var(--radius-pill);
    background: var(--bg-2); color: var(--fg-1); flex: 0 0 auto;
  }
  .chip.warn { background: var(--warn-soft); color: var(--warn); }
  /* The window's own controls, which act on the application rather than on the repository.
     Only the first is pushed away from the path; the rest sit against it. */
  .chrome {
    display: flex; align-items: center; justify-content: center;
    width: 26px; height: 26px; padding: 0; cursor: pointer;
    border-radius: var(--radius-1);
    border: 1px solid transparent; background: transparent; color: var(--fg-2);
  }
  .chrome:hover { background: var(--bg-2); border-color: var(--border); color: var(--fg-0); }

  /* The two faces occupy the same 16 pixels; only their rotation says which is showing. */
  .swap { position: relative; }
  .face {
    position: absolute; inset: 0; display: grid; place-items: center;
    transition: transform var(--slower) var(--ease-spring), opacity var(--slow) var(--ease);
  }
  .swap .moon { transform: rotate(0deg) scale(1); opacity: 1; }
  .swap .sun { transform: rotate(-90deg) scale(0.4); opacity: 0; }
  .swap.dark .moon { transform: rotate(90deg) scale(0.4); opacity: 0; }
  .swap.dark .sun { transform: rotate(0deg) scale(1); opacity: 1; }

  @media (prefers-reduced-motion: reduce) {
    .face { transition: none; }
  }
</style>
