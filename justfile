set shell := ["bash", "-euo", "pipefail", "-c"]

# `env_var` fails the whole justfile when its variable is missing, which on Windows broke every
# recipe rather than only the build ones.
home := env_var_or_default("HOME", env_var_or_default("USERPROFILE", "."))

# cargo is not on the non-interactive PATH on every machine; see docs/DECISIONS.md. Windows is
# the exception: rustup puts it there, and its separator is `;`.
export PATH := if os_family() == "windows" { env_var("PATH") } else { home + "/.cargo/bin:" + env_var("PATH") }
export RUST_BACKTRACE := "1"
export CARGO_BUILD_JOBS := env_var_or_default("CORAL_JOBS", num_cpus())

kernel := env_var_or_default("CORAL_KERNEL_REPO", home + "/.cache/coral-bench/linux")

default: check

# The gate. Everything must be green before a commit.
check:
    #!/usr/bin/env bash
    set -uo pipefail
    # Two lanes: cargo holds an exclusive lock on the target directory, so only the npm half can
    # run alongside. The bindings cross between them, which is why the Rust lane asserts the
    # committed copy is current — that is what makes svelte-check's read of it valid.
    ui_log="$(mktemp)"
    trap 'rm -f "$ui_log"' EXIT

    just ui-check >"$ui_log" 2>&1 &
    ui=$!

    rust=0
    just fmt-check && just lint && just test && just bindings-current && just licenses-drift \
        || rust=$?

    wait "$ui"; ui_status=$?
    echo
    echo "── ui-check ─────────────────────────────────────────────"
    cat "$ui_log"

    [ "$rust" -ne 0 ] && echo "the rust lane failed" >&2
    [ "$ui_status" -ne 0 ] && echo "the ui lane failed" >&2
    [ "$rust" -eq 0 ] && [ "$ui_status" -eq 0 ]

# The same checks one after another, when interleaved output is in the way of a failure.
check-serial: fmt-check lint test bindings-current ui-check licenses-drift

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Also regenerates ui/src/ipc/types.ts, via ts-rs.
test:
    cargo test --workspace --all-features

ui-check:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="$HOME/.cargo/bin:$PATH"
    cd ui
    npm run check
    npm test
    npm run build
    # Both of these fail only at runtime, so the built output is searched for them: Svelte's
    # server entry opens a blank window, and the preview fixtures once shipped in a release.
    if grep -rql "is not available on the server" dist/assets; then
        echo "ui build resolved Svelte's server entry; the window would open blank" >&2
        exit 1
    fi
    if grep -rql "Ada Lovelace" dist/assets; then
        echo "ui build contains the preview fixtures" >&2
        exit 1
    fi

bindings-drift: test bindings-current

bindings-current:
    git diff --exit-code -- ui/src/ipc/types.ts

dev:
    cd crates/coral-app && cargo tauri dev

# The application binary alone, built the way a bundle is. `cargo build --release` is not this.
app:
    cd crates/coral-app && cargo tauri build --no-bundle

# Throws away what this project builds, keeps what it downloads.
clean-artefacts:
    #!/usr/bin/env bash
    set -euo pipefail
    # Every build runs this first: the frontend is embedded at compile time, so a stale ui/dist
    # is a stale application however fresh the Rust is.
    root='{{ justfile_directory() }}'
    cargo clean --manifest-path "$root/Cargo.toml" \
        -p coral-core -p coral-hosting -p coral-cli -p coral-app 2>/dev/null || true
    rm -rf "$root/ui/dist" "$root/target/release/bundle"

# The shippable bundles for whichever platform this is.
build *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    # Named up front because both tools exit 127 with nothing else, which after two minutes of
    # compiling is not an answer, and npm's would be buried under the cargo output.
    missing=()
    if ! command -v npm >/dev/null 2>&1; then
        missing+=("npm, from Node 24. The interface is built with Vite.")
        missing+=("     Installed through nvm? A recipe does not see it: nvm is set up by an interactive shell profile.")
    fi
    if ! cargo tauri --version >/dev/null 2>&1; then
        missing+=("cargo-tauri. Install it with: cargo install tauri-cli --version '^2' --locked")
    fi
    if [ "${#missing[@]}" -gt 0 ]; then
        echo "just build needs a tool this machine does not have:" >&2
        printf '  %s\n' "${missing[@]}" >&2
        echo "docs/build/os.md lists everything a build needs, for all three platforms." >&2
        exit 1
    fi
    just clean-artefacts
    ( cd ui && npm ci ) &
    npm_pid=$!
    # The CLI ships beside the application, so it has to exist before the bundle is assembled.
    cargo build --release -p coral-cli
    wait $npm_pid
    cd crates/coral-app && cargo tauri build {{ARGS}}

# The oldest glibc a Linux binary will run on, which nothing else reports.
glibc-floor binary:
    #!/usr/bin/env bash
    set -euo pipefail
    highest=$(objdump -T '{{binary}}' | grep -oP 'GLIBC_\K[0-9.]+' | sort -uV | tail -1)
    echo "{{binary}} needs glibc >= ${highest}"
    echo "this machine has $(ldd --version | head -1 | grep -oP '[0-9]+\.[0-9]+$')"

# Universal, which is what a .dmg should carry. Needs both apple-darwin targets installed.
build-mac:
    just build --target universal-apple-darwin

# WebView2 is assumed present: it ships with Windows 11 and every supported Windows 10.
build-windows:
    just build

# Windows from Linux, via cargo-xwin. For checking it still compiles, not for a release.
build-windows-cross:
    #!/usr/bin/env bash
    set -euo pipefail
    # Only NSIS comes out — the MSI bundler needs Windows — and it cannot be signed from here.
    missing=()
    command -v cargo-xwin  >/dev/null || missing+=("cargo install cargo-xwin --locked")
    command -v lld-link    >/dev/null || missing+=("apt install lld llvm")
    # Ubuntu ships clang-cl as a driver mode of clang rather than as its own binary.
    command -v clang-cl    >/dev/null || missing+=("ln -s \"\$(command -v clang)\" ~/.local/bin/clang-cl")
    command -v makensis    >/dev/null || missing+=("apt install nsis")
    rustup target list --installed | grep -qx x86_64-pc-windows-msvc \
        || missing+=("rustup target add x86_64-pc-windows-msvc")
    if [ ${#missing[@]} -ne 0 ]; then
        echo "cross-compiling for Windows needs:" >&2
        printf '  %s\n' "${missing[@]}" >&2
        exit 1
    fi
    cd ui && npm ci && cd ..
    cd crates/coral-app
    cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis

build-all:
    @echo 'One machine each: run `just build` on Linux, macOS and Windows.'
    @echo 'Windows can also be cross-compiled from Linux: `just build-windows-cross`.'
    @echo 'CI does all three on its own runners: see .github/workflows/release.yml.'

cli *ARGS:
    cargo run -q -p coral-cli -- {{ARGS}}

# Full clone; a shallow one breaks topological order and invalidates every graph benchmark.
kernel-clone:
    mkdir -p "$(dirname '{{kernel}}')"
    [ -d '{{kernel}}' ] || git clone https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git '{{kernel}}'
    git -C '{{kernel}}' config gc.auto 0
    git -C '{{kernel}}' commit-graph write --reachable --no-progress

kernel-test: kernel-clone
    CORAL_KERNEL_REPO='{{kernel}}' bash tests/kernel/run.sh

# Exists so IDE run configurations need no shell expansion.
open-kernel:
    cargo run --release -p coral-cli -- --repo '{{kernel}}' --json open

licenses:
    cargo about generate about.hbs > THIRD_PARTY_LICENSES.md

# Fails if a dependency was added without regenerating the licence file.
licenses-drift: licenses
    git diff --exit-code -- THIRD_PARTY_LICENSES.md

deny:
    cargo deny check
