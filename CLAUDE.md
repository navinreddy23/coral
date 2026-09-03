# Working rules

## Structure

- One noun per module; the file name is the noun. No `utils.rs`, `helpers.rs`, `misc.ts`, `common/`.
- Functions do one thing and fit on a screen. Split at ~40 lines.
- The public API surface of `coral-core` is the contract for both the CLI and the app. Add to it deliberately.
- No abstraction for a single use. Introduce a trait or generic when the second implementation exists.
- Types document shape; make illegal states unrepresentable (enums over string tags, newtypes for `Oid`, `RefName`, `RepoPath`).

## Comments

- Code says what; comments say why, and only when the why is not obvious: a git quirk, a platform limitation, a performance trade-off. Link the git doc or issue.
- No comment that restates the signature, the type, or the next line. No section banners. No commented-out code. No `TODO` — open an issue or do it.
- No doc comments on private items. On public items, one line only if the name does not carry it.

## Rust

- `unsafe_code = "deny"`, `clippy::pedantic` with a short, justified allow list in the workspace manifest, rustfmt defaults.
- `thiserror` errors with a `code()` for the JSON envelope; no `anyhow` in `coral-core`.
- No `unwrap`/`expect` outside tests; no `panic!` on user data.
- Bytes for paths and git output until the display boundary.
- Every git command string lives in the module that owns the operation, built with the `GitCommand` builder — never string-concatenated.

## TypeScript

- `strict`, no `any`, no `!` non-null assertions.
- Components (`.svelte`) render; state lives in `.svelte.ts` modules using `$state`/`$derived`; `ipc/` talks to Rust. Nothing else calls `invoke`.
- Runes only: no legacy `$:` statements, stores, or `export let`. Props via `$props()`, events via callback props.
- Generated `ui/src/ipc/types.ts` is never hand-edited.

## Process

- Tests first for parsers and layout. Fixture test for every operation before it gets a UI.
- `just check` (fmt, clippy, tests, svelte-check, build) passes before every commit.
- Commits are small and named for the behaviour they add. Conventional prefixes: `core:`, `cli:`, `app:`, `ui:`, `hosting:`, `test:`, `docs:`.
- When you copy or adapt an approach, snippet, or algorithm from another project, blog, or paper, add it to `CREDITS.md` in the same commit with a link and license.
- Keep `docs/ARCHITECTURE.md` at one page and current.
- Never run `git gc`, `repack`, `prune`, or `reflog expire` on a user's repo.

## Environment note

`cargo` is not on the non-interactive `PATH` on every machine. The `justfile` prepends
`~/.cargo/bin`; scripts and CI steps must do the same rather than assume `cargo` resolves.
