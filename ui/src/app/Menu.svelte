<script lang="ts" module>
  /** One row of a context menu. A separator carries nothing but its kind. */
  export type MenuItem =
    | { kind: 'separator' }
    | {
        kind: 'item';
        label: string;
        /** Shown dimmed after the label, for a keystroke or a hint. */
        hint?: string;
        /**
         * Whether this is the choice currently in force, for a menu that offers a set of
         * them — a group's colour, a way of pulling. Drawn as a tick in the leading gutter,
         * which is where every other menu on the desktop puts it.
         */
        checked?: boolean;
        /**
         * A colour this item stands for, drawn as a disc before the label.
         *
         * For a menu whose subject is the colour itself: reading the word "Olive" and
         * imagining it is the one thing such a menu should not ask.
         */
        swatch?: string;
        disabled?: boolean;
        /** Marks a destructive choice, which is drawn in the danger colour. */
        danger?: boolean;
        run: () => void;
      }
    | {
        kind: 'submenu';
        label: string;
        items: MenuItem[];
      };
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import { fitPanel, fitSubmenu } from './placement';

  const { x, y, items, onClose }: {
    x: number;
    y: number;
    items: MenuItem[];
    onClose: () => void;
  } = $props();

  let panel = $state<HTMLDivElement | null>(null);
  /** Which submenu is open, by index, so only one is ever showing. */
  let open = $state<number | null>(null);

  /**
   * Where the panel actually sits.
   *
   * Measured after it is in the DOM rather than guessed from an assumed size: the menu is as
   * tall as its longest list, and one opened near the bottom of the window would otherwise
   * have its last items — the destructive ones — off screen and unreachable.
   */
  let fitted = $state<{ left: number; top: number } | null>(null);
  const at = $derived(fitted ?? { left: x, top: y });
  $effect(() => {
    if (!panel) return;
    const box = panel.getBoundingClientRect();
    fitted = fitPanel(x, y, box, viewport());
  });

  /**
   * Where the open submenu sits.
   *
   * The stylesheet hangs it off the right of its row, which is right until the menu itself is
   * against the right edge of the window: the panel is fitted there, the submenu that hangs
   * off it was not, and its rows ran off the window with no way to read or reach them — Reset,
   * whose three choices differ only in how much they throw away, was exactly that menu.
   *
   * Measured from the row rather than from the submenu's own box, so running it again after a
   * flip reaches the same answer instead of oscillating.
   */
  let sub = $state<HTMLDivElement | null>(null);
  let subFit = $state<{ left: string; top: string } | null>(null);
  $effect(() => {
    void open;
    const el = sub;
    const row = el?.parentElement ?? null;
    if (el === null || row === null) {
      subFit = null;
      return;
    }
    subFit = fitSubmenu(
      row.getBoundingClientRect(),
      { width: el.offsetWidth, height: el.offsetHeight },
      viewport(),
    );
  });

  function viewport() {
    return { width: window.innerWidth, height: window.innerHeight, margin: 8 };
  }

  function choose(item: Extract<MenuItem, { kind: 'item' }>) {
    if (item.disabled) return;
    onClose();
    item.run();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<!--
  The backdrop takes every pointer event, so one click anywhere dismisses the menu and cannot
  also press whatever was underneath it.
-->
<div
  class="scrim"
  role="presentation"
  onclick={onClose}
  oncontextmenu={(e) => {
    e.preventDefault();
    onClose();
  }}
></div>

<div
  class="menu"
  bind:this={panel}
  style:left="{at.left}px"
  style:top="{at.top}px"
  role="menu"
  tabindex="-1"
>
  {#each items as item, i (i)}
    {#if item.kind === 'separator'}
      <div class="rule" role="separator"></div>
    {:else if item.kind === 'submenu'}
      <div
        class="wrap"
        role="presentation"
        onmouseenter={() => (open = i)}
        onmouseleave={() => (open = open === i ? null : open)}
      >
        <button class="row" aria-haspopup="true" aria-expanded={open === i}>
          <span class="tick"></span>
          <span class="label">{item.label}</span>
          <span class="more"><Icon name="chevronRight" size={13} /></span>
        </button>
        {#if open === i}
          <div
            class="sub"
            role="menu"
            bind:this={sub}
            style:left={subFit?.left ?? 'calc(100% - 2px)'}
            style:top={subFit?.top ?? '-5px'}
          >
            {#each item.items as child, j (j)}
              {#if child.kind === 'item'}
                <button
                  class="row"
                  class:danger={child.danger}
                  disabled={child.disabled}
                  title={child.hint ? `${child.label}\n${child.hint}` : child.label}
                  onclick={() => choose(child)}
                >
                  <span class="tick">
                    {#if child.checked}<Icon name="check" size={12} />{/if}
                  </span>
                  {#if child.swatch}
                    <span class="swatch" style:background={child.swatch}></span>
                  {/if}
                  <span class="label">{child.label}</span>
                  {#if child.hint}<span class="hint">{child.hint}</span>{/if}
                </button>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    {:else}
      <!--
        A row caps its width, so a long label or a hint that is a sentence rather than a
        keystroke can be cut. Whatever the cap takes is on the row itself.
      -->
      <button
        class="row"
        class:danger={item.danger}
        disabled={item.disabled}
        title={item.hint ? `${item.label}\n${item.hint}` : item.label}
        onclick={() => choose(item)}
      >
        <span class="tick">{#if item.checked}<Icon name="check" size={12} />{/if}</span>
        {#if item.swatch}<span class="swatch" style:background={item.swatch}></span>{/if}
        <span class="label">{item.label}</span>
        {#if item.hint}<span class="hint">{item.hint}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 60; }
  .menu {
    position: fixed; z-index: 61; min-width: 15em; max-width: 32em;
    /* Padding all round rather than only top and bottom, so a hovered row is a rounded chip
       inside the panel rather than a band running into its edges. */
    padding: var(--space-1);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-2);
    /*
     * The one place a shadow is worth its cost: a menu floats over content it must be legible
     * against, and a border alone leaves it looking pasted onto the page.
     */
    box-shadow: var(--elevate-2);
    font-size: var(--text-base);
  }
  .wrap { position: relative; }
  .row {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; font: inherit; font-size: var(--text-base);
    padding: 5px var(--space-2); cursor: pointer;
    background: var(--bg-0); border: 0; border-radius: var(--radius-1); color: var(--fg-0);
    transition: background var(--fast) var(--ease);
  }
  .row:hover:not(:disabled), .row:focus-visible:not(:disabled) {
    background: var(--accent-soft);
  }
  .row:disabled { color: var(--fg-2); cursor: default; }
  .row.danger { color: var(--danger); }
  .row.danger:hover:not(:disabled) { background: var(--danger-soft); }
  /*
   * The label says what the item does and the hint only qualifies it, so the hint gives way
   * first. Both shrinking together cut "Checkout v1.0" to "Check…" beside a hint that had the
   * whole row to itself.
   */
  .label {
    flex: 0 1 auto; min-width: 0; margin-right: auto;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /* The colour itself, before its name. The ring is what keeps a pale one visible on the
     panel's own ground, which is nearly white in the light theme. */
  .swatch {
    flex: 0 0 auto; width: 11px; height: 11px; border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 18%);
  }
  .hint {
    flex: 0 8 auto; min-width: 0; color: var(--fg-2); font-size: var(--text-sm);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /*
   * Turned around where the row is disabled. There the hint is not a qualifier, it is the
   * reason the row cannot be used, and giving way first cut it to "feature/good-…", which is
   * the half the reader already had from the label.
   */
  .row:disabled .label { flex-shrink: 8; }
  .row:disabled .hint { flex-shrink: 0; }
  .more { flex: 0 0 auto; color: var(--fg-2); display: flex; }
  /* Reserved on every row, ticked or not, so a menu where one item is in force does not
     indent that row alone. */
  .tick {
    flex: 0 0 auto; width: 14px; display: flex; align-items: center;
    color: var(--accent);
  }
  .rule { height: 1px; margin: var(--space-1) var(--space-2); background: var(--border); }
  /* Opens to the right of its parent row, overlapping it by a pixel so the pointer can cross
     between the two without passing over the page and closing it. */
  .sub {
    position: absolute; left: calc(100% - 2px); top: -5px; min-width: 12em;
    padding: var(--space-1);
    background: var(--bg-0); border: 1px solid var(--border-strong);
    border-radius: var(--radius-2); box-shadow: var(--elevate-2);
  }
</style>
