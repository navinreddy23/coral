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
  credit the influence.
- **gitk** and `git log --graph` — prior art for commit graph lane assignment.
- **lazygit** and **gitui** — prior art for driving the git binary from a client.
- **Git Credential Manager** — the pattern of a binary acting as its own credential helper.

## Shell and interface

- **Tauri** — the desktop shell. <https://tauri.app/> — MIT OR Apache-2.0.
- **Svelte**, **Vite**, **TypeScript** — the interface layer.
- **Lucide** icons — ISC. **Inter** — OFL. **JetBrains Mono** — OFL.

## Rust libraries

tokio, notify, reqwest, rustls, serde, thiserror, clap, keyring, secrecy, bstr, similar, insta,
tracing, ts-rs, tempfile, url, smallvec. Full license texts are generated into
`THIRD_PARTY_LICENSES.md` by `cargo about`.
