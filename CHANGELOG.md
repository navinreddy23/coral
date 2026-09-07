# Changelog

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
