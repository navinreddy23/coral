# Credits

Coral is built on other people's work. This file is maintained from the first commit, not
retrofitted: when an approach, snippet, or algorithm is adapted from another project, blog, or
paper, it is added here in the same commit with a link and its license.

## The engine

- **Git** and its contributors — the engine Coral drives. The porcelain v2 format, the diff3
  hunking, and the credential helper protocol are all git's designs.
  <https://git-scm.com/> — GPL-2.0.
- **gitoxide** (Sebastian Thiel and contributors) — in-process object and graph reads.
  <https://github.com/GitoxideLabs/gitoxide> — MIT OR Apache-2.0.

## Interaction design

- **GitKraken** by Axosoft — the interaction design this project imitates. Not affiliated with
  Axosoft; no GitKraken assets, icons, logo, or CSS are used, and the name appears here only to
  credit the influence. The keyboard shortcuts follow the published GitKraken Client Cheat
  Sheet so that someone who already uses it does not have to relearn anything, and the graph's
  node shapes — a ring for a commit, filled for a merge, dotted for the working tree — follow
  the same document. <https://www.gitkraken.com/>
- **gitk** and `git log --graph` — prior art for commit graph lane assignment.
- **lazygit** and **gitui** — prior art for driving the git binary from a client.
- **Git Credential Manager** — the pattern of a binary acting as its own credential helper.

## Shell and interface

- **Tauri** — the desktop shell. <https://tauri.app/> — MIT OR Apache-2.0.
- **Svelte**, **Vite**, **TypeScript** — the interface layer.
- **Inter** by Rasmus Andersson — the interface face, bundled. Only the latin subset of the
  variable weight axis ships. <https://rsms.me/inter/> — SIL Open Font License 1.1, delivered
  through `@fontsource-variable/inter`.
- **JetBrains Mono** by JetBrains — the face every object id, path and diff is set in,
  bundled. Only the latin subset at 400 and 700 ships.
  <https://www.jetbrains.com/lp/mono/> — SIL Open Font License 1.1, delivered through
  `@fontsource/jetbrains-mono`.
- **xterm.js** — the terminal pane. <https://xtermjs.org/> — MIT.

## Not shipped, but recognised

- **git** — GPL-2.0-only. No build of Coral ships a copy. The application will run an
  unmodified git placed beside its own binary, for a packager who needs one on machines whose
  git predates the 2.40 the engine needs; Preferences, Experimental offers it only when one is
  actually there, and names the licence and the source.
  <https://mirrors.edge.kernel.org/pub/software/scm/git/>

## Rust libraries

tokio, notify, reqwest, rustls, serde, thiserror, clap, keyring, secrecy, bstr, similar, insta,
tracing, ts-rs, tempfile, url, smallvec. Full license texts are generated into
`THIRD_PARTY_LICENSES.md` by `cargo about`.
