# Coral

A fast Git client. Rust engine, Tauri 2 shell, Svelte 5 interface, and a CLI that exposes the
same engine so everything can be driven and tested headlessly.

The target is the Linux kernel: 1.4M commits and a 96k-file worktree, opening in under
300 ms and scrolling at 60 fps.

## Layout

| Path | What it is |
|---|---|
| `crates/coral-core` | The engine. Everything git. |
| `crates/coral-hosting` | GitHub and GitLab. |
| `crates/coral-cli` | Binary `coral`. The headless test surface. |
| `crates/coral-app` | Tauri backend. |
| `ui/` | Svelte 5 + Vite interface. |

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
