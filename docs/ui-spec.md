# Interface

Coral's design, and the reasons for it. This is a description of what the window is, not a
target to be measured against something else — an earlier version of this file asked for
parity with another client, and the interface has since been designed on its own terms.

## What it is trying to be

A precision instrument. The window is read for hours at a time on repositories with a million
commits in them, so the chrome is quiet and the data is loud: near-monochrome surfaces, hairline
separation, one accent spent only where something is selected or focused, and colour reserved
for the places it means something.

Three rules follow from that and are enforced by tests rather than by intention.

**One accent, one brand.** Aqua does every functional job — what is selected, what has focus,
what a primary button is. Coral carries the identity, and appears in exactly four places: the
mark, the line along the top of the current tab, the working copy's row, and the boot screen.
They are kept apart by role rather than by hue, because a warm red cannot mean "brand" in one
place and "this deletes something" in another.

**Nothing is written in pixels.** Colour, type, spacing, shape and motion come from
`ui/src/styles/tokens.css` and nowhere else. `ui/tests/style.test.ts` fails a component that
writes a literal colour, a type size, a shadow outside an overlay, or a transition timed by
hand.

**Every ratio is measured.** `ui/tests/contrast.test.ts` asserts the contrast of every token
pair the window actually renders, in both themes, against the panel rather than the page —
most of the dim text in this window is on `bg-1`, where a ratio computed against white
flatters itself by half a point.

## Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ ⌘ Coral ⟨Personal⟩ ⟦coral⟧ ⟦linux⟧ ⟦notes⟧ +      ⌄ ⚙ ☾ │ ─ □ ✕         │
├─────────────────────────────────────────────────────────────────────────┤
│ repo › branch │ ↶ ↷ │ ⟳ ↓▾ ↑▾ │ ⑂ ⤓ ⤒ ▤                              >_ │
├──────────────┬──────────────────────────────────────────┬───────────────┤
│ ⌕ filter…    │  ⟦main⟧ ●  a summary            2h  ab12c │ Commit detail │
│ ⌄ Local    3 │        │╲                                 │      or       │
│   ✓ main     │        │ ●  another summary     3h  9f1e2 │ Staging panel │
│   feature/x  │        ● │                                │               │
│ › Remote   2 │                                           │               │
│ ⌄ Stashes  1 │                                           │               │
│ › Tags   944 │                                           │               │
└──────────────┴──────────────────────────────────────────┴───────────────┘
```

Left and right panels collapse; widths persist per repository. The theme follows the desktop
unless it is told not to. Panel splitters are draggable, and the two that size the commit
list's columns are invisible until the pointer is on them — there is no line to draw between
the branch pills and the nodes they point at without cutting the one thing that ties them
together.

## Title bar

Coral draws its own, and the window opens without the desktop's. The tab strip is the title
bar: the mark and the wordmark at the leading edge, the tabs, then the log, preferences and
theme buttons, then minimise, maximise and close. Forty pixels, where the desktop's bar,
Coral's own header row and a separate tab strip took a hundred and twenty between them.

- Empty strip moves the window. Double-clicking it does nothing: a tab strip is somewhere
  people click twice by accident, and a window that jumps to full screen for it is worse than
  a gesture nobody has.
- The window's three buttons are full height and the last one ends at the corner, so the
  pointer cannot overshoot them.
- Four pixels along each edge and twelve at each corner resize the window, which is what the
  desktop's border used to do.
- The path is not in the strip. It is on the toolbar's `repository` crumb as a tooltip, and on
  each tab's own tooltip.
- Right-clicking the strip offers the desktop's title bar back, for a window manager that
  handles an undecorated window badly. The choice persists.
- Preferences and the activity log fill the window under the strip, never over it: with no
  desktop title bar, covering it would leave no way to move or close the window. The log is
  reached from the corner of the status bar, beside the work it reports on rather than beside
  the buttons that close the window.
- A profile chip sits between the wordmark and the tabs, named and on one of the eight lane
  colours. Clicking it lists the profiles and offers the Preferences pane that manages them.

## Transfers

Fetch, push and clone show a strip above the panes, never over them: waiting is long enough
without the window covering what somebody was reading to say so. It carries the phase, a bar,
the counts, and a Stop.

The bar is determinate only once git has counted something. Before that it is a moving stripe,
because a bar at nought per cent claims progress it has no basis for, and that is exactly the
state a host which never answers stays in. Stop is offered from the first moment for the same
reason.

## Toolbar

Forty pixels, left-aligned, and icons alone. Undo and Redo, disabled when the journal is empty;
Fetch, Pull with a caret for how it integrates, Push with a caret for what it sends; Branch,
Stash, Pop, and Patch, which writes the commits between two picked ones out as a series or
takes a patch file in. The terminal toggle sits at the far end, because it is the one control
on the row that acts on the window rather than on the repository.

The words are in the tooltips rather than on the buttons: the name first, then what the action
does, then its keystroke where it has one — read from the same binding table the shortcut sheet
is built from, so a shortcut is written down once. Every button also carries an accessible
name, which is now the only name it has.

The graph filter is not here. It is opened over the list it searches with `Ctrl+F`, with a
result count and up and down through the matches.

## Sidebar

Sections are named in sentence case with a drawn icon and a count, collapse, and remember their
state; a search field at the top filters all of them, with the magnifier inside the box rather
than beside it — a field labelled only by its placeholder loses that label the moment somebody
types in it. Every row keeps a gutter for the tick that marks the branch you are on, whether it
is ticked or not, so four sections share one left edge. Rows have
hover actions. The current branch is bold with a check mark. Remote rows show a host icon and
the signed-in avatar when a hosting provider matches, with branches nested beneath. Pull
requests group by remote showing number, title, author avatar and state, and a local branch's
menu offers to open one on a form naming the branch it would land on, with a title, a
description and a draft switch. That entry appears only where a host is signed in to talk to,
and the branches it offers are the remote's: a branch this machine has never pushed is not one
the host can be asked to merge into. Tags sort by version.
Stashes show message and age. Submodules are listed only.

Every branch and tag row carries an eye on its right, beside the dots, taking that ref out of
the walk and putting it back. It stays showing on a hidden row rather than waiting for a hover,
since otherwise nothing on screen says a branch is missing from the graph. A soloed repository
shows a banner above the filter box naming the branch with a Leave button, the soloed row
marked and every other row dimmed; a repository with refs merely hidden shows a quieter count
and a Show all. A row outside the current walk stays clickable, and clicking it widens the view
rather than doing nothing.

The push button's caret offers an ordinary push, a force push, and a push that carries the
tags. A push git refuses is not the end of the road: the rejection is shown with git's own
reason and the two ways out of it, pulling and rebasing or forcing. Forcing always asks first
and is always `--force-with-lease`, so it refuses when the remote has moved since Coral last
fetched and cannot overwrite a change it has not seen.

Context menus — local branch: checkout, rename, delete, push, pull, set upstream, merge into
current, rebase current onto, create PR/MR, hide, solo, copy name. Remote branch: checkout
(creating a tracking local), delete from remote, merge, rebase, create PR/MR. Remote: fetch,
push the current branch to it, edit, remove, prune, open on web. Pushing to a remote that is
not the upstream names the branch, since git will not work it out for a remote the branch has
never been to, and leaves the upstream where it was. Tag: checkout, push, delete, copy. Stash: apply, pop, drop,
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

Columns: branch and tag pills, lanes (canvas), then the message with the relative date and
short object id at its trailing edge. No column headers — they named three columns whose
contents are a pill, a drawing and a sentence, and cost twenty-six pixels of every screen.

Row height follows the density chosen in Preferences: 24, 28 or 32 pixels, with 28 shipped. The
same three numbers reach the stylesheet and the canvas metrics, which cannot read each other, so
`ui/tests/density.test.ts` reads both files and fails when they drift.

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
count, description textarea, an amend checkbox that fills the message in from the commit it
would replace and takes it back if it is unticked untouched, and "Commit changes to N files"
(Ctrl/Cmd+Enter).
During an operation the button becomes "Commit and merge" / "Continue rebase" with an Abort
link.

## Diff view

Inline and side-by-side toggle, hunk headers with stage/unstage/discard
hunk, line selection to stage/unstage/discard lines, whitespace toggle, file history and blame
tabs, and next/previous change in both layouts. The unified view can be widened from the hunks
to the whole file; side by side always shows the whole of it. Large and binary files render as a placeholder
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

## Cloning

URL, destination, and the name it lands under. For a URL that will be reached over ssh — a
scheme or git's scp-like `user@host:path` — the form also offers which key to authenticate
with, defaulting to the current profile's and to the agent otherwise. It is not offered for
https, which authenticates through the credential helper and never consults a key. The key is
passed to the clone and then written into the new repository, so every later fetch uses it.

Coral runs git with no terminal and no askpass, so the form says what that means: a key with a
passphrase has to be in the agent already, or the clone fails rather than asking.

How much to take is one choice rather than two switches, because the two economies are
different and nobody wants to reason about both at once. Everything is the default. Recent
history only is `--depth` on a single branch, which the graph walker grafts at its boundary.
History now, file contents on demand is `--filter=blob:none`: every commit and every tree
arrive, the graph is complete, and reading an old file needs the network.

## Signing in to a host

The host chip at the right of the status bar names GitHub or GitLab and says when there is no
token. It is a button: it opens the account, where a token is pasted and stored for the profile
at the window, so a work account and a personal one on the same host are both signed in and
neither can see the other's token. The dialog says which of the three states it is in, because
the third one — falling back to the token the command line stored, which every profile shares —
is the one where signing out affects the other profiles.

## Terminal

The shell is whatever `$SHELL` names, started interactively so the prompt, aliases and
completions are the ones already configured. Preferences names the shell it would use and
takes another, and says whether the login files are read. That is on by default only on macOS,
where `/etc/zprofile` runs `path_helper` and a shell that skips it has half a `PATH`; a Linux
desktop has already read them for the session Coral was started in.

## Command palette and shortcuts

`Ctrl/Cmd+P` opens the palette with every action, fuzzy search and recent items. Defaults:
`Cmd+T` new tab, `Cmd+W` close tab, `Cmd+Z` undo, `Cmd+Shift+Z` redo, `Cmd+Enter` commit,
`Cmd+F` filter, `Cmd+,` preferences, `Cmd+Shift+P` push, `Cmd+Shift+L` pull, `Cmd+B` new
branch, `Cmd+Shift+G` add current tab to a group, `Cmd+1…9` jump to tab, `Ctrl+Tab` next tab.
All rebindable.

## Profiles

Work repositories and personal ones on one machine, without either showing while the other is
being worked on. A profile owns the open tabs, the groups, the active tab and the recent list,
and carries a name, an email address, an ssh key and signing settings.

- Switching puts one workspace away and takes the other out. Everything the window is showing
  belongs to the outgoing repository and goes with it: the graph, the sidebar, the watcher, the
  terminal and the settings page.
- The identity is written into a repository's own config when one is cloned or created under
  the profile, so git and the command line see exactly what Coral does. A repository that was
  merely opened is never written to; the Profiles pane says who it commits as and offers one
  button to apply the profile there.
- Managed in Preferences: add, rename, recolour from the eight lane colours, remove. The last
  profile stays, since its tabs would have nowhere to go. Removing one forgets what it had
  open and touches no repository.
- Which branches are hidden, which git Coral runs, and every view preference stay outside a
  profile. The first belongs to the repository, the second to the installation, the third to
  whoever is at the window.

## Tabs

Shaped like a browser's: rounded above, flared at the base, and the current one wearing the
toolbar's own fill so the two read as one surface, with an accent line along its top edge.
Neighbours are parted by a hairline that fades wherever a fill already separates them.

Each tab carries a picture, which is a repository's nearest thing to a favicon. It is a branch
unless the tab's context menu was used to pick another from the twelve on offer, and it is drawn
in a colour hashed from the path, so a strip of tabs is as scannable as a browser's without
anyone having chosen anything. The choice persists per tab.

## Tab groups

Chrome's tab groups applied to repositories. Tabs stay individual; a group is a named, coloured
tray around a contiguous run of them, with its name on a chip at the leading edge.

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
- The new-tab button sits after the last tab and sticks to the trailing edge once the strip
  overflows, so it is adjacent where there is room and reachable where there is not. The tab
  search stays outside the scrolling strip: it exists for the case where there are too many
  tabs to look through, so it must never be among them.
- Groups, tab order, colour, collapsed state and the active tab persist and restore on launch.
  A group whose repos are gone restores with those tabs marked missing, not dropped. Closing a
  group keeps it in "Recently closed groups".

Every open tab owns an engine, its watcher, and its graph rows. Tabs in a collapsed group keep
their engines and watchers alive so badges stay current. Graph chunks for tabs not shown in ten
minutes are dropped and rebuilt from the Rust row store on the next switch; the row store
itself is never rebuilt on a tab switch.

## The design system

`ui/src/styles/tokens.css` is the only place colour, spacing, type, shape and motion live.

**Surfaces** are a four-step ladder biased cool: the page a list sits on, the panels either
side of it, a control at rest, and one under the pointer. Depth is layered surfaces and
hairlines. A shadow appears only on something that floats free of the page and never scrolls —
a menu, a dialog, the palette, a toast — because a shadow on a row is a large soft fill
repainted on every scroll frame, and this window scrolls a million rows.

**Every surface carrying text paints its own opaque background.** WebKit antialiases text on a
composited layer with subpixel precision only where it knows what is behind it, so
`background: none` silently drops it to grayscale. See `docs/ARCHITECTURE.md`. The lane canvas
is the one deliberate exception.

**Type** is Inter for the interface and JetBrains Mono for anything git produced, both bundled
as latin subsets — 96 KB for three files. Ligatures are off: JetBrains Mono draws `!==` as one
glyph, and in a diff a reader checking whether a line says `!=` or `!==` should not have to know
how the face draws them. Six sizes and no more, plus one for lettering inside a disc or a badge,
which is sized to the shape holding it rather than to the reading scale.

**Icons** are one drawn set on a 24-unit grid, in `ui/src/app/icon.ts`. Stroke weight is not
stored with them — `Icon.svelte` derives it from the size asked for, so a glyph on an
eleven-pixel branch pill and one in a twenty-pixel button land on the same optical weight.

**The mark** is one commit, the two branches leaving it, and the trunk carrying on past them:
a commit graph and a piece of coral, which is a thing that grows by branching. Four nodes, not
three, because three made a Y and a Y is the branch glyph. `Mark.svelte` is the one drawing;
the seven bundled application icons are generated from the same shape.

**The theme** follows the desktop and keeps following it, so a desktop that goes dark in the
evening takes Coral with it. The switch in the title bar sets light or dark outright;
Preferences, Appearance is where the choice is handed back.
