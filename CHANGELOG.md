# Changelog

## 1.0.1

Two lives on one machine, a repository taken without all of it, and a menu that offers what
can actually happen. As with 1.0.0, nearly all of it came from driving the window rather than
from reading the code.

### The window

- **Profiles.** Work and personal on one machine: each keeps its own tabs, groups and recent
  repositories, and its own identity for repositories cloned under it. Switching points the
  workspace at another directory rather than teaching the tabs about profiles.
- **A work account and a personal one on the same host.** Hosting tokens were filed under the
  host's origin alone, so github.com held exactly one. They now carry the profile, and a
  profile that has never signed in falls back to the one the command line writes. The status
  bar's host chip opens the sign-in, which the window had no way to reach at all.
- **Watch a fetch, a push or a clone, and stop it.** Progress reaches the window, cancelling
  reaches the git it started, and a cancelled operation is recorded as cancelled rather than
  as a failure.
- **Choose the shell the terminal opens**, and whether it reads the login files. A shell
  already running keeps what it started with, so the pane has a button that starts it again.
- **Take a repository without all of it.** The clone form and `coral clone` offer recent
  history only, which is a depth, and history now with file contents on demand, which is a
  blobless filter. They are different economies and are one choice rather than two switches.

### Correctness

- **Undo reaches a commit**, which is the commonest thing anyone wants back. It behaves as a
  soft reset: the branch moves, the work returns to the index.
- **A pinned ssh key lost to one named in the user's own config.** `-i` and a `Host` block's
  `IdentityFile` accumulate and the agent reorders them, so the wrong account authenticated.
  Only `-F none` makes the choice authoritative.
- **A stash that will not apply has stopped, not failed.** git exits 1 for a conflict and for
  a real failure alike; the repository, not the exit code, tells them apart. The merge tool
  opens on it, and hides Continue and Abort where git has no operation to continue.
- **A moved tag could not be published.** `--force-with-lease` is compared against the
  remote-tracking ref and git keeps none for a tag, so every forced tag push was refused. The
  lease now names what the remote holds.
- **A ref the branch has passed offered every way back to it.** Right-clicking a release tag
  on an up-to-date branch offered a fast-forward git refuses. The menu asks where the ref
  stands and turns the fast-forward around; a branch is taken along without being checked out,
  a tag is replaced, and where neither can happen the line says which way round they are.
- **The graph went stale after a pull**, and blank after a tag was fast-forwarded from far
  down the history. Both were the reload deciding whether to keep the rows on screen.

### Saying what happened

- A rejected push said it stopped on conflicts, in a status line that phrased outcomes itself
  rather than asking the module that owns the wording. Both now say the same thing.
- The rejection dialog explained a tag as though it were a branch, and offered to pull, which
  does nothing about a name the remote already has.
- A modified run in the diff overview strip was drawn in a graph lane colour. It is now the
  two colours the diff itself uses.
- `coral clone` and `coral init`, so the command line can do everything the window can.

## 1.0.0

Coral draws its own title bar, renames a branch, deletes a tag on its remote, and takes back a
reset. Most of what follows it, though, came from driving the window as a user rather than as
a test: scenarios on real repositories, on the kernel, on shapes nobody sets out to build, and
on two developers sharing one remote.

### The window

- **One row where there were three.** The desktop's title bar, Coral's own header and the tab
  strip each took a row to say the window was called Coral. The tabs are the title bar now, at
  forty pixels, carrying the mark, the tabs, the log, preferences and theme buttons, and
  minimise, maximise and close. Dragging is handled in the page so a stray double click does
  not throw the window to full screen, and right-clicking the strip hands the bar back to the
  desktop for a window manager that handles an undecorated window badly.
- **Rename a branch.** The engine and the CLI have always been able to; the window could not.
- **Delete a tag on its remote**, which was the missing half of pushing one.
- **Undo a reset.** A soft or mixed reset leaves the worktree dirty by construction, and undo
  refused on a dirty worktree, so the action people most want back was the one they could never
  have. The guard now asks whether the restore would overwrite anything rather than whether
  anything is dirty — including whatever is staged, which is a place work can exist in only one
  copy.
- **A repository with nothing in it says so**, instead of an empty pane beside a panel offering
  to show the details of a commit that could be selected.
- The start page is a column with a measure in the middle of the window, rather than a strip in
  one corner of a large empty rectangle.

### Correctness

- **A generated patch quotes a path the way git quotes it.** Staging or discarding a hunk in a
  file whose name holds a newline or a tab built a header git could not read, so it staged
  nothing — or, worse, silently lost half the name.
- **A conflicted path is read rather than refused.** Opening a conflicted file showed
  "malformed git diff output" where the conflict should have been.
- **Finishing a conflicted merge or rebase is journalled**, so undo can take it back instead of
  reaching past it to an older entry and failing in git's words under a button marked Undo.
- **A write waits out an index lock another git holds for a moment.** The file watcher runs a
  status on every change, and a status beside a commit made the commit fail.
- **A password in a remote URL never reaches a message.** The argv was always redacted; git's
  own output was not.
- The last row of the graph can be scrolled to — on the kernel that is its first commit — and
  the commit message column can no longer be squeezed out of existence in a narrow window.

### Saying what happened

- An operation that stopped is no longer logged as finished, so a rejected force push does not
  read like one that went through.
- A failure explains itself when git explains itself on stdout rather than stderr, which is
  what a conflicting stash pop does.
- Object ids in labels are short everywhere, conflicted files are listed once rather than twice,
  a waiting toast no longer sits on the commit button, and the fast-forward menu hint no longer
  reads as a refusal of the thing it is about to do.

## 0.2.0

The release that made the graph somebody else's repository rather than only a well-behaved one,
and gave the window the operations that were still only in the engine.

### The graph

- **Solo and hide.** A branch or tag can be walked on its own, or taken out of the walk
  entirely; either choice is remembered per repository. The panel says which, with a banner
  naming the soloed branch and an eye on every row that stays showing once a ref is hidden.
- **Shallow clones open.** A clone with no history past its boundary used to fail outright,
  which is every module in a `west` workspace. The boundary is grafted the way git grafts it.
- **Drag and drop between pills.** A branch onto its tracking branch pushes, the other way
  round pulls, and anything else asks whether to merge, rebase, or open a request.
- **Delete a branch from a remote** from its own row, behind a dialog that says the deletion
  is for everyone and that nothing local is touched.
- **A rejected push offers a way out.** The window had no force at all, so a non-fast-forward
  rejection could only be resolved in a terminal. It now shows git's reason and offers to pull
  and rebase or to force, and the push menu carries a deliberate force as well. Always
  `--force-with-lease`.
- **Search by path.** The filter now also finds commits by the files they touched, alongside
  the message, the author and the object id, and the four passes run together.
- **Tags newest first**, by version rather than by name, so a repository with a thousand of
  them opens on the ones somebody wants.
- The lane colour runs from a commit's node to its message rather than tinting the message
  itself, so the selected row is findable again on a coloured graph.

### Working with changes

- **Patches.** Two commits picked write out the series between them; the same button takes a
  series back in, either as commits keeping their authors or as changes left in the working
  copy. A patch that no longer applies stops in the conflict tool.
- **Amending starts from the message it replaces** instead of an empty box.
- **A unified diff widens to the whole file**, and steps change by change in both layouts.
- **An embedded terminal**, opening in the repository, on Ctrl+`.

### Hosting

- **Open a pull or merge request** from a branch, on a form that names where it would land,
  with a title, a description and a draft switch.

### The window

- **Tabs look like a browser's**: rounded above, flared at the base, the current one merging
  into the toolbar. Each carries a picture — a branch unless another of twelve is picked from
  its menu — coloured from the repository's path so a strip of tabs is scannable.
- **Every keyboard shortcut the panel lists now does something.** It listed several that did
  not.
- **About** says which build this is: version, commit and whether it is a debug build.
- Commit signing configured per repository and inherited from the app.

### Fixed

- Clicking a commit's message did nothing; only the author's initials selected the row.
- A collapsed tab group's count was invisible, painted in the strip's grey on the strip's grey.
- The breadcrumb cut the tails off `g`, `p` and `y` and the foot off a slash.
- One repository's commit could appear under another repository's name.
- Transfer progress overwrote rebase progress.
- A long line pushed the other pane off the side in side-by-side.

## 0.1.0

The first version: the graph, the operations, conflicts, remotes, hosting, tabs and the CLI.
