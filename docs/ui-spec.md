# UI specification

Fidelity target: someone who uses GitKraken daily should be able to sit down and use Coral
without reading anything. Match layout, density, colours, interactions, and iconography style.
Do not copy GitKraken's assets, logo, name, icons, or CSS; use Lucide icons, Inter, and
JetBrains Mono.

## Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ ☰  [kernel ▾][linux][stable][mods]  [tools ▾]  [repo tab]  [+]  ⚙ avatar │
├─────────────────────────────────────────────────────────────────────────┤
│ ↶ ↷ │ Pull ▾   Push   Branch   Stash   Pop                   🔍 filter   │
├──────────────┬──────────────────────────────────────────┬───────────────┤
│ filter…      │  graph │ message + labels │ avatar │ date │ sha           │
│ ▾ LOCAL      │  // WIP  3 files  +1 ~2 -0               │ Commit detail │
│   ✓ main     │  ●──  Merge tag 'v6.x' of …   ⊙  2h  ab12c│  or           │
│   feature/x  │  │╲                                       │ Staging panel │
│ ▾ REMOTE     │  │ ●  net: fix …                ⊙  3h  9f1e2│              │
│ ▾ PULL REQ.  │  ● │  …                                   │               │
│ ▾ TAGS       │                                           │               │
│ ▾ STASHES    │                                           │               │
│ ▾ SUBMODULES │                                           │               │
└──────────────┴──────────────────────────────────────────┴───────────────┘
```

Left and right panels collapse; widths persist per repo. Default dark theme; light theme
included. Panel splitters are draggable.

## Toolbar

Undo and Redo, disabled when the journal is empty, with a tooltip naming the operation. Pull
with a dropdown: fast-forward if possible (default), fetch all, pull (rebase), pull (merge).
Push. Branch, creating at the selected commit or HEAD with an inline name input in the graph.
Stash. Pop. On the right, a graph filter searching message, author, SHA and file paths (`-S`
via `git log`), with a result count and up/down navigation.

## Sidebar

Sections collapse and remember state; a filter box at the top filters all of them. Rows have
hover actions. The current branch is bold with a check mark. Remote rows show a host icon and
the signed-in avatar when a hosting provider matches, with branches nested beneath. Pull
requests group by remote showing number, title, author avatar and state. Tags sort by version.
Stashes show message and age. Submodules are listed only.

Every branch and tag row carries an eye on its right, beside the dots, taking that ref out of
the walk and putting it back. It stays showing on a hidden row rather than waiting for a hover,
since otherwise nothing on screen says a branch is missing from the graph. A soloed repository
shows a banner above the filter box naming the branch with a Leave button, the soloed row
marked and every other row dimmed; a repository with refs merely hidden shows a quieter count
and a Show all. A row outside the current walk stays clickable, and clicking it widens the view
rather than doing nothing.

Context menus — local branch: checkout, rename, delete, push, pull, set upstream, merge into
current, rebase current onto, create PR/MR, hide, solo, copy name. Remote branch: checkout
(creating a tracking local), delete from remote, merge, rebase, create PR/MR. Remote: fetch,
edit, remove, prune, open on web. Tag: checkout, push, delete, copy. Stash: apply, pop, drop,
view diff.

## Patches

The toolbar's **P** does whichever of the two the selection asks for. Two commits picked means
"write out what lies between them": it says how many files that is before a directory is chosen,
since two commits far apart on a large repository is a file per commit between them. Anything
else means "take a patch in", through a file dialog that accepts a series. Applying asks whether
to record a commit per patch, keeping the author the file carries, or to leave the changes in
the working copy to read first. The same two sit on a commit's context menu.

A patch that no longer applies stops in the conflict tool like any other operation, named after
the branch and the patch rather than "ours" and "theirs", and is continued or abandoned there.

## Graph

Columns: lanes (canvas), message with inline label pills, author avatar, relative date
(absolute on hover), short SHA. Row height 28 px, compact 22 px.

- Canvas draws only visible rows plus one screen of overscan. Edges are Bézier curves between
  rows; lanes take a stable colour from the eight-colour palette in `tokens.css`. Commit nodes
  show the author avatar once loaded, otherwise a filled circle in the lane colour. The HEAD
  node is larger with a ring.
- Label pills sit on the row of their target. A local and remote branch of the same name
  collapse into one pill with a remote avatar. The HEAD pill is highlighted; a branch with an
  open PR gets a badge. Click selects, double-click on a branch pill checks out, hover
  highlights the lane, and pills are draggable.
- The WIP row appears at the top when the worktree is dirty, showing `+added ~modified
  -deleted` and a diff-stat sparkline. Selecting it opens the staging panel.
- Drag and drop: local pill onto local pill opens merge / rebase / create PR; local onto remote
  pushes; remote onto local pulls. Drop targets highlight.
- Right-click a commit: checkout, create branch here, create tag here, cherry-pick, revert,
  reset current branch to here (soft/mixed/hard), rebase current onto, merge into current, copy
  SHA, view details, hide/solo this branch.
- Hide/solo removes hidden tips from the walk or walks only that tip; state persists per repo.
- Keyboard: ↑↓ moves selection, Enter opens details, `/` focuses the filter, Esc clears.

## Right panel

**Commit details** — avatar, author, committer when different, relative and absolute dates,
full SHA with copy, parents as links, message with description, file list with tree/flat toggle
and a path filter, and `+ - ~` counts. Clicking a file opens the diff in the centre with a
breadcrumb back to the graph.

**Staging panel** (WIP selected) — "Unstaged Files" with "Stage all files", "Staged Files" with
"Unstage all files", each row with hover buttons to stage/unstage and to discard with
confirmation; a conflicted files section during operations. Below: summary input with character
count, description textarea, amend checkbox, and "Commit changes to N files" (Ctrl/Cmd+Enter).
During an operation the button becomes "Commit and merge" / "Continue rebase" with an Abort
link.

## Diff view

Inline and side-by-side toggle, word-level highlighting, hunk headers with stage/unstage/discard
hunk, line selection to stage/unstage/discard lines, whitespace toggle, expand context, file
history and blame tabs, next/previous change. Large and binary files render as a placeholder
with an explicit load action.

## Merge conflict tool

Full-width centre panel replacing the graph. Two source panes on top, each with a header
checkbox to take all and a checkbox per conflict block; the output pane below with an editor;
Save becoming Mark resolved per file; the right panel listing conflicted files with resolved
ticks. When every file is resolved the right panel offers "Commit and merge" / "Continue
rebase" / "Continue cherry-pick" with a message box, and Abort as a secondary action. Blocks
are colour-coded per side and the output highlights which side each line came from.

Labels are ref names, never "ours/theirs": for a merge, the current branch and the branch being
merged; for a rebase, the `onto` ref and the summary of the commit being replayed. State once,
in the UI, that during a rebase "current" is the branch being rebased onto — this confuses
everyone.

## Command palette and shortcuts

`Ctrl/Cmd+P` opens the palette with every action, fuzzy search and recent items. Defaults:
`Cmd+T` new tab, `Cmd+W` close tab, `Cmd+Z` undo, `Cmd+Shift+Z` redo, `Cmd+Enter` commit,
`Cmd+F` filter, `Cmd+,` preferences, `Cmd+Shift+P` push, `Cmd+Shift+L` pull, `Cmd+B` new
branch, `Cmd+Shift+G` add current tab to a group, `Cmd+1…9` jump to tab, `Ctrl+Tab` next tab.
All rebindable.

## Tab groups

Chrome's tab groups applied to repositories. Tabs stay individual; a group is a named, coloured
band around a contiguous run of them.

- Create from a tab's context menu, from `Cmd+Shift+G`, or by dragging one tab onto another and
  holding. A popover asks for a name and one of the eight lane colours.
- Tabs drag into, out of, and between groups; a group moves as a unit by its header. A tab
  dropped inside a group's band joins it; dropped outside, it leaves.
- Clicking the group header collapses it to a chip; opening a repo in a collapsed group expands
  it.
- Group context menu: rename, change colour, new tab in group, fetch all in group, ungroup,
  close group, move group to new window.
- The chip shows an aggregate badge — repos with uncommitted changes, and ahead/behind totals.
  Hovering lists each repo with its branch and counts.
- Fetch-all-in-group runs one `git fetch --all --prune` per repo, concurrency capped at 3, with
  a single combined progress toast.
- Groups, tab order, colour, collapsed state and the active tab persist and restore on launch.
  A group whose repos are gone restores with those tabs marked missing, not dropped. Closing a
  group keeps it in "Recently closed groups".

Every open tab owns an engine, its watcher, and its graph rows. Tabs in a collapsed group keep
their engines and watchers alive so badges stay current. Graph chunks for tabs not shown in ten
minutes are dropped and rebuilt from the Rust row store on the next switch; the row store
itself is never rebuilt on a tab switch.

## Theme tokens

`ui/src/styles/tokens.css` is the only place colours, spacing and type live. Tune by eye
against GitKraken's dark theme; never spread hex values through components.
