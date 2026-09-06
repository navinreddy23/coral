# Changelog

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
