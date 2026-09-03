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

**`--no-optional-locks` on `Read` but not on `Status`.** With that flag git cannot write the
refreshed index back, so the stat cache stays cold and every subsequent status re-hashes
racily-clean files. On an 80k-file worktree that costs more than the lock contention it avoids,
and the engine serializes writes anyway.

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
