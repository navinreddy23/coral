# Credits

Coral is built on other people's work. This file is maintained from the first commit, not
retrofitted: when an approach, snippet, or algorithm is adapted from another project, blog, or
paper, it is added here in the same commit with a link and its license.

## The engine

- **Git** and its contributors — the engine Coral drives. The porcelain v2 format, the diff3
  hunking, and the credential helper protocol are all git's designs, and the path the add-a-
  submodule form fills in from a URL follows `git_url_basename` in git's `dir.c`, so it is the
  one git would have chosen. <https://git-scm.com/> — GPL-2.0.
- **gitoxide** (Sebastian Thiel and contributors) — in-process object and graph reads.
  <https://github.com/GitoxideLabs/gitoxide> — MIT OR Apache-2.0.

## Interaction design

- **GitKraken** by Axosoft — the client whose interaction design Coral's feature set was
  originally laid out against. Not affiliated with Axosoft; no GitKraken assets, icons, logo or
  CSS are used, and the name appears here only to credit the influence. What remains of it is
  the keyboard shortcuts, which follow the published GitKraken Client Cheat Sheet so that
  somebody who already uses it does not have to relearn anything, and the node shapes in the
  graph — a ring for a commit, filled for a merge, dotted for the working tree. The visual
  design is Coral's own; see `docs/ui-spec.md`. <https://www.gitkraken.com/>
- **gitk** and `git log --graph` — prior art for commit graph lane assignment.
- **Pierre Vigier**, "Commit graph drawing algorithms" (2019) — the split of a commit's children
  into branch children and merge children, and with it where an edge changes lane: a merge's
  edge on the merge's row, a branch's on the row of the commit it left from, which is how the
  article finds GitKraken drawing straight branches. Written from the article's description;
  no code was taken, and the page states no licence.
  <https://pvigier.github.io/2019/05/06/commit-graph-drawing-algorithms.html>
- **lazygit** and **gitui** — prior art for driving the git binary from a client.
- **Git Credential Manager** — the pattern of a binary acting as its own credential helper.
- **WAI-ARIA Authoring Practices**, W3C — the tree view keyboard pattern the file lists and the
  branch list follow: ↑ and ↓ between rows, → to open a folder or step into it, ← to shut it or
  go up to it. <https://www.w3.org/WAI/ARIA/apg/patterns/treeview/> — W3C Software and Document
  License.

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
- **highlight.js** by Ivan Sagalaev and contributors — the grammars that colour code in a diff,
  a blame and a file. Only its core and twenty-five grammars ship, in a worker of their own.
  <https://highlightjs.org/> — BSD-3-Clause.

## Spelling

- **spellbook** (the Helix editor's contributors) — Hunspell's checking and suggestions, for the
  commit box's spelling check. <https://github.com/helix-editor/spellbook> — MPL-2.0.
- **SCOWL** by Kevin Atkinson — the US and British English Hunspell dictionaries, bundled as they
  are released in `crates/coral-core/dictionaries`, with their READMEs.
  <https://wordlist.aspell.net/> — the SCOWL licence, which asks only that its notice is kept.
  The affix files derive from Geoff Kuenning's Ispell and carry his BSD licence; both notices are
  in `THIRD_PARTY_LICENSES.md`.
- **textarea-caret-position** by Jonathan Ong and Dan Dascalescu — the list of a field's
  properties that decide where its text falls, which the underline layer copies onto itself so
  each word lands over the word beneath (`ui/src/app/underline.ts`).
  <https://github.com/component/textarea-caret-position> — MIT.

## Not shipped, but recognised

- **git** — GPL-2.0-only. No build of Coral ships a copy. The application will run an
  unmodified git placed beside its own binary, for a packager who needs one on machines whose
  git predates the 2.40 the engine needs; Preferences, Experimental offers it only when one is
  actually there, and names the licence and the source.
  <https://mirrors.edge.kernel.org/pub/software/scm/git/>

## Rust libraries

tokio, notify, reqwest, rustls, serde, thiserror, clap, keyring, secrecy, bstr, similar, insta,
tracing, ts-rs, tempfile, url, smallvec, spellbook. Full license texts are generated into
`THIRD_PARTY_LICENSES.md` by `cargo about`.
