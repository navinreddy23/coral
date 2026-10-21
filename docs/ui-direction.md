# The 2026 refresh: direction

Working notes for the interface refresh on `fix/ui-refresh`. Folded into `ui-spec.md` and
deleted when the refresh is done.

The rules in `ui-spec.md` stay: one accent and one brand, nothing written in pixels, every
contrast measured, depth from surfaces rather than shadows, and opaque text surfaces. The same
zones, controls, shortcuts and gestures stay where they are. What changes is the shape the
window is drawn in.

## What made it read as dated

- **Hard rules everywhere.** Every zone met its neighbour at a full-height line, so the window
  read as a spreadsheet of panes rather than as a workspace.
- **Solid branch pills** in eight saturated fills, competing with the lanes they label.
- **One weight of heading.** Section names, panel titles and labels were all sentence-case
  `base` text, so nothing told the eye where a panel began.
- **Square overlays.** Menus and the palette had small radii and dense rows, and looked like
  toolkit defaults.

## What replaces it

- **Frame and cards.** The window's ground is `--bg-1`. The commit list, the diff and merge
  views, and the right panel are cards on it: `--bg-0`, a hairline border and `--radius-3`,
  separated from the frame and from each other by `--space-2`. The sidebar sits directly on
  the frame, with no card and no rule. The splitters live in the gaps.
- **The graph is unchanged.** Right-angled elbows and an initialled disc on every commit,
  merges included. Curved edges with merges drawn as dots were tried and taken back at the
  owner's request. Only the initials changed: they are now set in Inter.
- **Tinted pills.** A branch or tag is labelled with its lane's soft tint, a lane-coloured
  hairline and lane-coloured text. Only the checked-out branch is solid. The pill is where
  the lane gets its name, not a second lane.
- **Section labels.** Sidebar sections, panel sections and dialog groups get a small uppercase
  label with tracking (`--text-xs`, `--weight-semibold`, `--tracking-label`) and a hairline
  rule after it. Panel titles stay sentence case at `--text-lg`.
- **Muted commit prefixes.** In a Conventional Commits prefix (`ui:`, `core:`), the part
  before the colon is set in `--fg-2` mono, so the summary itself leads.
- **Overlays.** Menus, the palette, dialogs and toasts use `--radius-3`, `--elevate-2`, 30px
  rows (`--control-h`), an uppercase group label, and hairlines between groups.
- **Preferences.** Each row has its title and a one-line description on the left and its
  control on the right, with a hairline between rows.
- **Controls.** Toolbar buttons are `--control-h` square with a `--radius-1` hover surface,
  grouped by hairline dividers. Segmented controls replace pairs of toggles.

## New tokens

`--radius-0` (3px, small marks), `--radius-3` (12px, cards and overlays),
`--weight-regular/medium/semibold/bold`, `--hairline`, `--tracking-label`, `--control-h`,
`--control-h-sm`, `--strip-h`, `--toolbar-h`, `--statusbar-h`. `style.test.ts` rejects a
literal radius or font weight from here on.

## Not taken

- **Column headers over the commit list.** The pills, lanes and summary are recognised by
  position.
- **Merging the title strip and toolbar into one bar.** They are two zones in the spec, and
  the toolbar's repository and branch crumbs need the room.
- **Floating the right panel over the list.** It is docked, and the list resizes around it.
- **Avatars fetched from a host.** Initials stay: nothing is sent anywhere to draw a commit.
