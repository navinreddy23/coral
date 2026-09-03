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
