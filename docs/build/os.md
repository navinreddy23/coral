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
cargo install just cargo-deny
cargo install cargo-about --features cli
cargo install tauri-cli --version "^2" --locked
```

`cargo-about` and `cargo-deny` are only for `just licenses` and `just deny`; the build does not
need them.

**Node does not come from cargo.** It is the one requirement in the table above that the two
lines here do not install, and the build needs it: `just build` runs `npm ci` and then has Vite
compile the interface that gets embedded in the binary. Install it per platform below, and
check both halves are on the PATH before building:

```
node --version    # 24 or newer
npm --version
```

Without it the build compiles all of Rust first and then stops with exit code 127 — `npm ci`
runs in the background beside cargo, so its "command not found" scrolls past under the compiler
output. `just build` checks for it up front and says so by name.

The Tauri CLI comes from cargo rather than from npm so there is one CLI to keep in step with
the `tauri` crate instead of two that can drift.

## Linux

Ubuntu and Debian, including 26.04:

```
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
  xdg-utils
```

On a distribution without apt, the same five by their own names: the WebKitGTK 4.1
development package, libxdo, the Ayatana appindicator, librsvg, and the XDG desktop
utilities. `build-essential`, `pkg-config`, `libssl-dev`, `curl`, `wget` and `file` are
assumed present.

The last of those is easy to miss because most desktops already have it: the AppImage
bundler copies `/usr/bin/xdg-open` into the AppDir, and without it `just build` gets as far
as the `.deb` and then stops.

Then Node. Ubuntu's own `nodejs` package has been older than 24 on every release so far, so
take it from NodeSource:

```
curl -fsSL https://deb.nodesource.com/setup_24.x | sudo -E bash -
sudo apt install nodejs
```

`sudo apt install nodejs npm` instead is fine wherever the distribution's own package is
already 24 or newer — `apt policy nodejs` says which it would install. nvm works too, with one
catch: nvm is set up by an interactive shell profile, so a `just` invoked from somewhere that
has not read that profile — an IDE's run configuration, a launcher, cron — will not find the
npm that works in your terminal.

```
just check     # the gate: fmt, clippy, every test, svelte-check, the interface build
just dev       # the application, with the interface served by Vite
just app       # the application binary alone, built the way a bundle is
just build     # the shippable bundles: .deb and .AppImage
```

An AppImage demands the glibc of the machine that built it, and glibc is forward compatible
only, so one built on 24.04 will not start on 22.04 or Debian 12 and says nothing about why.
Build on the oldest distribution you mean to support, and check what came out:

```
just glibc-floor target/release/coral-app
```

## macOS

Xcode's command line tools provide the compiler and the SDK; WebKit is part of the system, so
there is nothing to install for the webview.

```
xcode-select --install
rustup target add aarch64-apple-darwin x86_64-apple-darwin
brew install node
```

Homebrew's `node` is current, so it satisfies the 24 the interface is built with. Without
Homebrew, the installer from nodejs.org does the same job; `nvm install 24` also works, with
the profile caveat in the Linux section above.

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

Two things beyond the common list, plus Node:

- **The MSVC build tools.** The Visual Studio Build Tools with the C++ workload, which is what
  `rustup` selects by default on Windows.
- **WebView2.** Present on Windows 11 and on every supported Windows 10, so nothing is bundled
  and nothing needs installing on a current system.

Node, as everywhere:

```
winget install OpenJS.NodeJS.LTS
```

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

`cargo build --release -p coral-app` is **not** a substitute for either. The interface is
embedded by the Tauri CLI's build step, so a plain cargo release build asks the dev server for
its page. With no dev server the window opens and never paints. With one it fills and works
while showing source that is not in the binary, so anything checked through it proves nothing.

To look at a change in the real window without waiting for bundles:

```
just app       # target/release/coral-app, built the way a bundle is
```

## When something fails

**`cargo: command not found` from a recipe.** cargo is not on the non-interactive PATH on every
machine. The justfile prepends `~/.cargo/bin` for exactly this; any script or CI step that runs
cargo itself must do the same. On Windows rustup puts cargo on the PATH already, and the
justfile leaves it alone there.

**`webkit2gtk-4.1` not found on Linux.** The development package, not the runtime one. Check
with `pkg-config --modversion webkit2gtk-4.1`.

**The window opens empty.** A cargo-only release build, or a stale `ui/dist`. `just build` runs
`just clean-artefacts` first for this reason.

**The window works but shows the wrong thing, or a change you can see in `just dev` is missing
from the binary.** Also a cargo-only release build, with a dev server running. Check the size:
one built by the Tauri CLI is about 640 KB larger, because it carries the interface. Build it
with `just app`.

**The AppImage will not start on another machine.** Almost always glibc. `just glibc-floor
<binary>` says which version the binary demands and which this machine has.

**`just` fails on Windows before running anything.** It is being run outside a bash shell. Use
Git Bash.

## What CI does

`.github/workflows/ci.yml` runs the gate on all three platforms on every push, and asserts that
the generated `ui/src/ipc/types.ts` and `THIRD_PARTY_LICENSES.md` are unchanged.
`.github/workflows/release.yml` builds the bundles, one runner per platform, on a `v*` tag.
Both are the authority on the exact package list; this page follows them.
