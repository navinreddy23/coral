# Building Coral on Linux, macOS and Windows

Everything below is one project and one set of recipes. What differs per platform is the
webview the shell links against — WebKitGTK, WebKit, WebView2 — and the packages that provide
it. The README's four lines are the Linux short version; this is the whole of it, for all
three.

## What every platform needs

| Tool | Version | Why |
|---|---|---|
| Rust | 1.88 or newer | `rust-version` in `Cargo.toml`, edition 2024 |
| Node | 24 | What CI builds with; the interface is Vite and Svelte 5 |
| git | 2.40 or newer | The floor the engine committed to, checked at startup |
| `just` | any | The recipes below are its |
| Tauri CLI | 2.x | Embeds the interface into the binary and makes the bundles |

```
cargo install just cargo-about cargo-deny
cargo install tauri-cli --version "^2" --locked
```

`cargo-about` and `cargo-deny` are only for `just licenses` and `just deny`; the build does not
need them.

The Tauri CLI comes from cargo rather than from npm so there is one CLI to keep in step with
the `tauri` crate instead of two that can drift.

## Linux

```
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
```

On a distribution without apt, the same four by their own names: the WebKitGTK 4.1
development package, libxdo, the Ayatana appindicator, and librsvg. `build-essential`,
`pkg-config`, `libssl-dev`, `curl`, `wget` and `file` are assumed present.

```
just check     # the gate: fmt, clippy, every test, svelte-check, the interface build
just dev       # the application, with the interface served by Vite
just build     # the shippable bundles: .deb and .AppImage
```

An AppImage built here demands the glibc of the machine that built it, and glibc is forward
compatible only, so one built on 24.04 will not start on 22.04 or Debian 12 and says nothing
about why. For anything that leaves this machine:

```
just build-linux-portable            # in a container with an older glibc, needs Docker
just glibc-floor target/portable/release/coral-app
```

`just build-linux-portable git=bundled` puts a git of Coral's own inside the AppImage, for
machines whose git predates 2.40. It is off unless the user turns it on in Preferences.

## macOS

Xcode's command line tools provide the compiler and the SDK; WebKit is part of the system, so
there is nothing to install for the webview.

```
xcode-select --install
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

```
just check
just dev
just build-mac    # a universal .dmg
```

Both targets, always. An Intel-only bundle runs under Rosetta on Apple silicon, and a native
Apple silicon bundle will not start on an Intel Mac at all.

Signing and notarisation are not wired into the recipes. Without them macOS will refuse a
downloaded bundle until it is cleared in System Settings, which is expected for a local build.

## Windows

Two things beyond the common list:

- **The MSVC build tools.** The Visual Studio Build Tools with the C++ workload, which is what
  `rustup` selects by default on Windows.
- **WebView2.** Present on Windows 11 and on every supported Windows 10, so nothing is bundled
  and nothing needs installing on a current system.

The recipes are bash, which on Windows means the Git Bash that ships with git. Run `just` from
that rather than from PowerShell or `cmd`.

```
just check
just dev
just build-windows    # an NSIS installer
```

An MSI needs WiX and therefore a Windows host; NSIS is what the bundle configuration asks for
and what a Windows build produces.

### Cross-compiling Windows from Linux

For checking that the code still compiles for Windows without waiting on CI. Every dependency
of `coral-app` except `webkit2gtk` was once declared Linux-only, and nothing but a Windows
build says so.

```
cargo install cargo-xwin --locked
rustup target add x86_64-pc-windows-msvc
sudo apt install lld llvm nsis
ln -s "$(command -v clang)" ~/.local/bin/clang-cl
just build-windows-cross
```

Only NSIS comes out, the installer cannot be signed from here, and Tauri calls the arrangement
experimental. A release is built on a Windows runner.

## What a build produces

| Platform | Bundles | Where |
|---|---|---|
| Linux | `.deb`, `.AppImage` | `target/release/bundle/` |
| macOS | `.dmg` | `target/release/bundle/` |
| Windows | NSIS `.exe` | `target/release/bundle/` |

The `coral` CLI is built first and ships beside the application; the Linux `.deb` maps it onto
`/usr/bin/coral`.

`cargo build --release -p coral-app` is **not** a substitute for `just build`. The interface is
embedded by the Tauri CLI's build step, so a plain cargo release build produces a binary that
starts, opens a window, and never loads a page.

## When something fails

**`cargo: command not found` from a recipe.** cargo is not on the non-interactive PATH on every
machine. The justfile prepends `~/.cargo/bin` for exactly this; any script or CI step that runs
cargo itself must do the same. On Windows rustup puts cargo on the PATH already, and the
justfile leaves it alone there.

**`webkit2gtk-4.1` not found on Linux.** The development package, not the runtime one. Check
with `pkg-config --modversion webkit2gtk-4.1`.

**The window opens empty.** A cargo-only release build, or a stale `ui/dist`. `just build` runs
`just clean-artefacts` first for this reason.

**The AppImage will not start on another machine.** Almost always glibc. `just glibc-floor
<binary>` says which version the binary demands and which this machine has.

**`just` fails on Windows before running anything.** It is being run outside a bash shell. Use
Git Bash.

## What CI does

`.github/workflows/ci.yml` runs the gate on all three platforms on every push, and asserts that
the generated `ui/src/ipc/types.ts` and `THIRD_PARTY_LICENSES.md` are unchanged.
`.github/workflows/release.yml` builds the bundles, one runner per platform, on a `v*` tag.
Both are the authority on the exact package list; this page follows them.
