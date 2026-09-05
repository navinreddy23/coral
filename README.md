# Coral

A desktop Git client that stays quick on a repository nobody else is quick on. Rust engine,
Tauri 2 shell, Svelte 5 interface, and a command-line front end over the same engine, so
everything the window does can be driven and tested without one.

The measure is the Linux kernel: 1.48M commits and a 96k-file worktree. It opens to a painted
graph in well under a second, walks the whole history in about five, and scrolls at 60 fps.
Those are budgets rather than boasts — `docs/ARCHITECTURE.md` lists them, and the benchmarks
run against a real clone.

## What it does

**The graph.** Every ref, with lanes drawn on a canvas and the rows as text beside them, so a
million commits cost a window's worth of DOM and nothing more. Branch and tag labels sit on the
commit they name. A branch can be **soloed** to walk only its history, or **hidden** to take it
out of the walk, and either choice is remembered for that repository.

**The panel.** Local branches, remotes with their branches nested, pull and merge requests,
tags newest first, stashes, and submodules. Anything can be filtered, and every branch and tag
carries an eye that takes it out of the graph.

**Working with changes.** Stage and unstage by file, by hunk or by line; commit, amend, discard.
The file panel answers four questions about one file: what changed, who wrote each line, what
has touched it, and the same again with whitespace discounted. Diffs read inline or side by
side, with word-level highlighting.

**Rewriting history.** Merge, rebase, interactive rebase with a picker for reword, squash,
fixup, edit, drop and reorder, cherry-pick, revert, and reset. Every one of them is journalled,
so **undo** reverses an operation it knows nothing about by restoring the refs it moved — and it
syncs the worktree, so undoing does not leave the index describing a commit that is no longer
there.

**Conflicts.** A three-pane tool built from the index stages rather than parsed out of the
worktree file, so what you see does not depend on your `merge.conflictStyle`. The sides are
named after the refs involved, never "ours" and "theirs", because during a rebase those words
are backwards and the tool says so.

**Remotes and hosting.** Fetch, pull, push, prune, and per-ref rejection reporting. Forcing is
always `--force-with-lease`. Coral is its own git credential helper, so a token never appears in
a URL, a config file or an argument list. GitHub and GitLab, including Enterprise and
self-hosted, list their pull and merge requests beside the branches.

**Around the edges.** Repository tabs with Chrome-style groups, an embedded terminal, commit
signing configured per repository, SSH key selection, worktrees, patches, a command palette,
rebindable shortcuts, and light and dark themes.

## Two front ends, one engine

`coral-cli` is not a demo. It exposes the whole engine — fifty-two commands, from `open` and
`status` through `rebase`, `conflict-resolve`, `blame` and `graph` — and answers in a stable
JSON envelope with documented exit codes:

```
coral --repo /path/to/repo graph --json --solo refs/heads/main | jq .result.total
coral --repo /path/to/repo status --json
```

That is what makes the kernel scenarios testable without a GUI, and it is why the window and the
CLI cannot disagree about what an operation did.

## Layout

| Path | What it is |
|---|---|
| `crates/coral-core` | The engine. Everything git. No UI, no HTTP, no Tauri. |
| `crates/coral-hosting` | GitHub and GitLab. Never on the graph's critical path. |
| `crates/coral-cli` | Binary `coral`. The headless test surface. |
| `crates/coral-app` | Tauri backend. Thin wrappers over the engine. |
| `ui/` | Svelte 5 + Vite interface. Renders; never decides. |

## Getting started

```
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
cargo install just cargo-about cargo-deny
cargo install tauri-cli --version "^2" --locked

just check     # fmt, clippy, tests, svelte-check, ui build
just dev       # run the app
just cli open  # run the CLI against the current directory
```

`just kernel-clone` fetches the Linux kernel into `~/.cache/coral-bench/linux` for the
benchmarks; `just kernel-test` runs the scenarios against it.

Those four lines are Linux. `docs/build/os.md` has the whole of it for Linux, macOS and
Windows, including the bundles and what to do when a build fails.

See `CLAUDE.md` for the working rules, `docs/ARCHITECTURE.md` for the design, and
`docs/DECISIONS.md` for why things are the way they are.

## License

MIT. See `CREDITS.md` for the projects Coral builds on.
