# Decisions

Departures from the original design document, and choices it left open. Each entry says what
changed and why.

## Naming

**Coral, not Reef.** The repository and working directory were already `Coral`. Crates are
`coral-core`, `coral-hosting`, `coral-cli`, `coral-app`; the binary is `coral`; the benchmark
clone lives at `~/.cache/coral-bench/linux`.

## Toolchain

**TypeScript pinned to 6.0.3, not the current `latest` 7.0.2.** TS 7 is the native compiler
port. `svelte-check@4.7.6` declares `typescript: "^5.0.0 || ^6.0.0"` and `svelte2tsx@0.7.61`
declares `^4.9.4 || ^5.0.0 || ^6.0.0`; neither admits TS 7, and
[language-tools#3063](https://github.com/sveltejs/language-tools/issues/3063) (TS 7 crashes
svelte2tsx and svelte-check) is open. Since `svelte-check` is part of `just check`, TS 7 would
break the gate on day one. Revisit when TS 7.1 lands and that issue closes.

**`ts-rs` 12.0.1 for IPC types, not `tauri-specta`.** The only Tauri-2-compatible
`tauri-specta` is `2.0.0-rc.25`, and the line has been in release candidate for a long time.
More decisively,
[tauri-specta#170](https://github.com/specta-rs/tauri-specta/issues/170) — support for
`tauri::ipc::Request` / `tauri::ipc::Response` — has been open since April 2025 and is blocked
on an unscheduled Specta release. The graph rows go out as `tauri::ipc::Response`, so
`tauri-specta` cannot describe the command that matters most. `ts-rs` generates types only, has
a stable semver line, and cannot be broken by a Tauri release; the `invoke` wrappers are
hand-written in `ui/src/ipc/`, which the working rules require anyway.

**Bindings are generated from `coral-core`, not `coral-app`.** The types are defined in the
engine, and generating there means `ui/src/ipc/types.ts` can be produced without building the
Tauri crate or installing its system dependencies. Gated behind the `ts` feature;
`TS_RS_EXPORT_DIR` is set in `.cargo/config.toml`.

**Tauri CLI from cargo, not `@tauri-apps/cli`.** One CLI to keep in step with the pinned
`tauri` crate instead of two that can drift.

**No `src-tauri/` directory.** `tauri.conf.json` resolves paths relative to its own directory
and `beforeDevCommand` / `beforeBuildCommand` accept `{ script, cwd, wait }`, so `coral-app`
is a plain workspace member with `ui/` as a sibling.

**`coral-app` is not yet a workspace member.** It joins once `libwebkit2gtk-4.1-dev` is
installed; until then `members` lists the three crates that build everywhere, so the gate stays
green.

## Engine

**`--no-optional-locks` on `Read` but not on `Status`, measured.** With that flag git cannot
write the refreshed index back, so the stat cache never persists. On the kernel worktree, after
touching 25,000 files so their mtimes change but their contents do not:

| Status after a stale-mtime event | run 1 | run 2 | run 3 |
|---|---|---|---|
| `--no-optional-locks` | 2265 ms | 886 ms | 861 ms |
| default | 884 ms | **83 ms** | **84 ms** |

Every refresh would sit near the 1 s budget ceiling permanently instead of dropping to 83 ms.
The lock contention that flag avoids does not arise anyway, because the engine serializes
writes. `Read` keeps the flag: those commands never refresh the index, so they have nothing to
write back and should not fight the user's own terminal for `index.lock`.

**Minimum git 2.40**, per the design document. Ubuntu 22.04 ships 2.34, so that distribution
needs a backport; the floor is enforced at startup with a clear message rather than a parse
failure downstream.

**One filesystem watcher process-wide, not one per repository.** This machine's
`fs.inotify.max_user_instances` is 128, which a per-tab watcher would exhaust with enough open
repositories. `max_user_watches` is 524288, which is ample for the kernel tree.

## Interface

**Light is the default theme**, at the owner's request; the design document specified dark.
The choice is explicit rather than following `prefers-color-scheme`, so a preference is stable
across machines whose system settings differ. Dark is opt-in via `data-theme="dark"` on the
root element and is remembered in local storage; a webview with storage disabled still opens.
Lane colours are darkened for the light palette, since the dark set is illegible on white.

**Graph rows never become JavaScript objects.** They stay inside one decoded frame as typed
arrays: 1.4M row objects would cost hundreds of megabytes in the webview before any drawing.
Measured in the app against the kernel — first paint 4,096 rows in 90 ms, the full
topological walk of 1,481,528 rows in 5.0 s, 286 MB resident for the whole application.

**The frame layout is pinned by a golden fixture** at `ui/tests/fixtures/frame.bin`, written by
the Rust tests and read by the TypeScript ones. An encoder and decoder in one language prove
only self-consistency; the shared bytes are what make a layout change fail on both sides.

**Running `target/*/coral-app` directly is not a supported configuration.** A debug build loads
`devUrl`, so without a dev server the window shows "connection refused" and no command is ever
called. Use `just dev` (`cargo tauri dev`) for development and `cargo tauri build` for a bundle.

## Environment

**`cargo` is not on the non-interactive `PATH`.** It lives in `~/.cargo/bin`. The `justfile`
prepends it; CI and any script must do the same.

## Known, deferred

## Graph architecture, measured

Benchmarked on the 1,481,528-commit kernel clone with a commit-graph present (945 commit
tips), release build, this machine.

| Walk | Rows | Wall | Peak RSS |
|---|---|---|---|
| gix topo, all tips | 1,481,528 commits / 1,601,455 edges | **2,685 ms** | 437 MB |
| `git rev-list --topo-order --parents --all` | same | **2,860 ms** | 698 MB (its own process) |
| gix first-parent from HEAD | 4,096 | **4 ms** | — |
| gix commit-time, all tips | 4,096 | **6 ms** | — |
| gix topo, all tips | 4,096 | **1,489 ms** | — |

**gix wins the full walk and is kept as the default `CommitStream`.** It is marginally faster
than the subprocess and avoids a spawn plus a parse on the hottest path. The subprocess reader
stays implemented as the oracle the differential tests compare against, and as the fallback.

**Two-phase first paint is mandatory, not optional.** The design document listed it as something
to do only if the 300 ms budget was missed. It is missed, by 5×: topological order must
prepaint indegrees across the whole graph before it can emit its first row, so the first 4,096
rows cost the same 1.5 s as the first million. The engine therefore paints a commit-time walk
first (6 ms for a screenful across all tips) and swaps in the topological rows when the full
walk lands ~2.7 s later. Commit-time order can misorder a child before its parent under clock
skew; the swap is what makes it correct, so the first paint must be visibly provisional in the
row store rather than treated as final.

**The subprocess fallback can replace gix for the full walk, but not for first paint.**
Measured through the `CommitStream` trait: a 4,096-row commit-time walk costs gix 54 ms and
`git rev-list --date-order --max-count=4096 --all` 2,424 ms, because rev-list still resolves and
orders all 945 tips before it emits anything. Both agree exactly on the full graph
(1,481,528 commits, 1,601,455 edges) at 2,733 ms and 2,991 ms respectively. So if gix ever has
to be swapped out, first paint needs its own strategy — first-parent from HEAD alone — rather
than the same query with a row limit.

**The row store costs 60.7 bytes per row; the build peak is what threatens the budget.**
Building the whole kernel graph — walk, lane assignment, parent resolution, lookup index —
takes 4.2 s consistently against the 5 s budget, so roughly 1.5 s on top of the raw walk. The
resulting store is 85.8 MB for 1,481,528 rows and 1,601,455 edges, at a maximum width of 214
lanes.

| | |
|---|---|
| Peak RSS during the build | 554 MB |
| RSS once the walk state is dropped | 248 MB |
| The store itself | 85.8 MB |

Most of the 248 MB that remains is the commit-graph and pack indexes, which are file-backed and
reclaimable, so the anonymous figure is far lower — this is why the budget has to be stated as
anonymous RSS to mean anything. The problem is the 554 MB *peak*: add WebKit's roughly 250 MB
and a first graph build transiently exceeds the 600 MB budget even for a single repository. The
walk state is transient, so steady state is comfortable; M5 has to decide whether to accept the
spike, build before the webview is heavy, or shrink the walk state.

**Parents are stored CSR, not per-row.** `SmallVec<[u32; 2]>` is 24 bytes on x86-64, so 1.48M
of them would cost 35 MB and scatter across the heap; the CSR offset/flat pair costs 12 MB and
scans linearly. `SmallVec` stays where it belongs, as the transient per-node parent list during
the walk. Object ids are stored packed at `hash_len` bytes each rather than as a fixed 20, so
SHA-256 repositories work without a second code path, and row lookup is a binary search over a
sorted index (6 MB) rather than a `HashMap<ObjectId, u32>` (about 60 MB).

**The 437 MB peak is the binding memory constraint.** That is the walk's own transient state
(indegree and flag maps plus the priority queues), not the row store, and it is freed when the
walk ends. Against a 600 MB total budget it means graph builds must be serialized across tabs:
two concurrent kernel-sized topo walks would alone exceed the budget. The tab-group requirement
that three kernel repos stay within three times the budget therefore constrains *concurrent
builds*, not open tabs.

**`gix` cannot write a commit-graph** (gitoxide `crate-status.md` lists graph writing as
unimplemented), and it has no reachability bitmap support. Repo-open therefore shells out to
`git commit-graph write --reachable` in the background when the graph is missing. The topo walk
API is `gix::traverse::commit::topo::Builder` with `Sorting::TopoOrder` — *not*
`gix::revision::walk::Sorting`, which has no topological variant. Whether it meets the 5 s
budget on the kernel is unmeasured and is the first thing M1 must benchmark against the
`git rev-list` fallback.

**Tauri's `tracing` feature must stay off in release builds.** Its IPC spans serialize the raw
request and response bodies, which would turn every binary graph chunk into a multi-megabyte
debug string.

**The IPC binary path needs a startup self-test.** If `fetch('ipc://…')` throws once, Tauri's
JS sets `customProtocolIpcFailed` permanently and falls back to `postMessage`, which serializes
`Vec<u8>` as a JSON array of numbers. That degradation is silent and roughly 100× slower, so
M5 must assert a known-size binary round trip at startup and fail loudly.

## The release binary comes from `cargo tauri build`, never `cargo build --release`

The frontend is embedded by the Tauri CLI's build step. A plain `cargo build --release -p
coral-app` compiles without complaint and produces a binary that asks `devUrl` for its page.

With no dev server running that window opens and never paints, and no IPC call is ever made, so
there is nothing in the log to explain it. **With one running it is worse**, and this is the part
that cost a day: the window fills, works, and shows whatever is in `ui/src` at that moment. It is
then a release build of source it does not contain, and every check made through it is worthless.
A fix looks confirmed while the binary a user runs still carries the bug.

It also hides everything a packaged build does differently. The page is served over the custom
protocol under the policy in `tauri.conf.json`, and the dev server applies none of it — which is
how a policy that refused every runtime stylesheet was verified as working, three times.

`just build` produces the bundles. `just app` is the same compilation without the bundling, for
looking at a change in the real window; it is the cheapest thing that is still honest. Neither
claim above is inferred: both were checked by binary size against the same freshly built
`ui/dist`, 644 KB apart, which is the bundle.

## `style-src` is exempt from Tauri's policy rewriting

Tauri appends a nonce to every CSP directive it is allowed to modify. A nonce in a directive
makes `'unsafe-inline'` inert — that is the CSP rule, not a Tauri quirk — so the configured
`style-src 'self' 'unsafe-inline'` arrived as `style-src 'self' 'unsafe-inline' 'nonce-…'` and
meant the opposite of what it says. Every stylesheet the page wrote at runtime was refused, with
`el.sheet` null and the rules never created.

xterm styles the terminal with exactly one such stylesheet, built from the theme and font it is
handed, so the terminal drew in the page's proportional face with no colour. Nothing else in the
window was affected: it is all styled from the bundled file, which `'self'` allows. The palette
was never wrong, and reading it back at runtime proved it — twenty entries, correct values, none
of them appliable.

`dangerousDisableAssetCspModification` is a list naming `style-src` alone, never `true`. A
blanket exemption would also drop the `script-src` hash, which is the directive actually holding
the window shut. `crates/coral-app/tests/csp.rs` asserts both halves, because the failure is
invisible to every test that does not run a packaged build.

## Soloing a branch walks that branch and nothing else

Including HEAD alongside the soloed tip was the other candidate. Solo exists to answer "show me
only this", and a second line of history nobody asked for makes it a two-branch graph, leaving
the reader to work out which of the two they soloed. So the soloed ref is the whole tip set,
and the branch you are standing on is off the screen until you leave. The banner names the
soloed branch and offers the way out in one click, so what is missing is both explained and
one click from returning.

The uncommitted-work row is the exception, and it stays. It is not drawn on a commit — it is a
sticky row above the list, owned by the working tree rather than by the walk — so nothing about
solo takes it away, and it names the branch it belongs to. Removing it would mean losing the
staging panel, and therefore the ability to commit at all, for as long as a different branch
was soloed. Showing it costs nothing: it reads "WIP on <branch>", which is the same sentence
whether or not that branch is in the graph beneath it.

Hiding is the opposite case and keeps HEAD: hiding a spike should not take the branch you are
standing on off the screen. HEAD is dropped there only when the branch it is on is itself the
one hidden, since otherwise the eye on your own branch would appear to do nothing.

## Hidden and soloed refs are stored per repository, in Rust

The rest of the window's preferences live in `localStorage` under the rule stated at the top of
`ui/src/state/views.svelte.ts`: they belong to the person at the window, not to the
repositories open in it. Which branches are hidden is the other kind. A dead spike is a fact
about that repository, and someone opening it on a second machine is not asking for their
spikes back. So it goes in `scope.json` beside `session.json`, keyed by repository path, and
the CLI can be given the same selection with `--solo` and `--hide`.


## The graph filter searches paths, not diff content

The interface specification asked for `-S`, git's pickaxe. It answers a different question from
the one a filter is asked. `-S` searches the *content* of every diff for a string, so searching
`sidebar` returns every commit that added or removed that word anywhere, and it walks every
commit and every diff to do it — minutes on a repository of any size, against the seconds the
other three passes take.

What somebody typing a file name into a filter means is "which commits touched this", and a
pathspec answers exactly that from the index the walk already has. `:(icase)*query*` matches any
part of any path, case-insensitively, the way the message and author passes match.

Searching diff content is worth having one day, but as its own thing with its own affordance and
its own warning about the cost, not folded into a filter that is expected to answer while you
type.

## A shallow clone is grafted rather than refused or shelled out to

gix's traversal does not read `.git/shallow`, so a shallow clone's boundary commit sent the
walk after a parent that was never fetched and the graph came back as an error. Three ways out
were on the table.

Refusing to draw a shallow repository was never one worth having: `west` clones every module in
a Zephyr workspace one commit deep, so that is a whole class of real workspace where most tabs
would show nothing.

Falling back to `git rev-list` for these repositories would have worked — git grafts natively,
and a shallow clone is small by definition, so the subprocess cost would not have mattered. It
was rejected because it makes the subprocess stream a production path rather than the oracle it
is, and then two implementations have to agree about every later feature rather than one being
checked against the other.

Grafting at the point the walker reads an object is what git itself does, costs a copy of one
commit, and leaves a single implementation in production. The one thing it forces is putting
the commit-graph aside while grafting, since the graph records the parents the clone does not
have; git refuses to write a commit-graph for a shallow clone for exactly that reason, so no
real repository loses anything by it.
