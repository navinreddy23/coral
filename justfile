# Recipes are bash everywhere, which on Windows means Git Bash — already present wherever
# Coral can be built at all, since it ships with git.
set shell := ["bash", "-euo", "pipefail", "-c"]

# `HOME` outside a Unix shell is `USERPROFILE`, and neither is guaranteed. `env_var` fails the
# whole justfile when its variable is missing, so every recipe stopped working on Windows —
# not merely the build ones.
home := env_var_or_default("HOME", env_var_or_default("USERPROFILE", "."))

# cargo is not on the non-interactive PATH on every machine; see docs/DECISIONS.md. Windows is
# the exception: rustup puts it there itself, and the separator is `;` rather than `:`, so
# prepending a Unix path there would corrupt the PATH rather than extend it.
export PATH := if os_family() == "windows" { env_var("PATH") } else { home + "/.cargo/bin:" + env_var("PATH") }
export RUST_BACKTRACE := "1"

# How many jobs cargo may run at once. It already defaults to one per core; this exists so a
# machine doing something else can be told to leave some, and so CI can pin it.
export CARGO_BUILD_JOBS := env_var_or_default("CORAL_JOBS", num_cpus())

kernel := env_var_or_default("CORAL_KERNEL_REPO", home + "/.cache/coral-bench/linux")

default: check

# The gate. Everything must be green before a commit.
#
# Two lanes at once. Every cargo command takes an exclusive lock on the target directory, so
# those cannot overlap each other — measured: running two of them concurrently only makes the
# second wait, and says so. Nothing in the npm half touches that lock, and the npm half is the
# whole of `ui-check`, so that is the lane worth running alongside.
#
# The generated bindings are the one thing that crosses between the lanes: `cargo test`
# regenerates `ui/src/ipc/types.ts`, and `svelte-check` reads it. Running them at once would
# type-check against whichever copy happened to be on disk, so the Rust lane also asserts the
# committed copy is the current one — which is what makes checking against it valid.
#
# The Rust lane streams, because it is the long one and the one worth watching. The npm lane is
# held back and printed when it finishes, so the two cannot interleave into nonsense.
check:
    #!/usr/bin/env bash
    set -uo pipefail
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

# The same checks one after another, for when interleaved output is in the way of reading a
# failure.
check-serial: fmt-check lint test bindings-current ui-check licenses-drift

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# `cargo test` also regenerates ui/src/ipc/types.ts via ts-rs.
test:
    cargo test --workspace --all-features

ui-check:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="$HOME/.cargo/bin:$PATH"
    cd ui
    npm run check
    npm run build
    # A bundle that resolved Svelte's server build instead of its browser one compiles without
    # complaint and then throws the moment it loads, leaving the window blank. Nothing else in
    # the gate notices, because the failure is at runtime, so the built output is searched for
    # the error it would raise.
    if grep -rql "is not available on the server" dist/assets; then
        echo "ui build resolved Svelte's server entry; the window would open blank" >&2
        exit 1
    fi
    # The browser fixtures must never reach a release. They are behind a branch the bundler
    # compiles away, but a static import kept the module anyway and a build once shipped with
    # invented commit messages in it. The name of a fixture author is the marker.
    if grep -rql "Ada Lovelace" dist/assets; then
        echo "ui build contains the preview fixtures" >&2
        exit 1
    fi

# Fails if the generated bindings drift from the Rust types. Runs the tests first, which is
# what regenerates them.
bindings-drift: test bindings-current

# The same assertion without rerunning the tests, for a caller that has just run them.
bindings-current:
    git diff --exit-code -- ui/src/ipc/types.ts

dev:
    cd crates/coral-app && cargo tauri dev

# Builds the shippable application for whichever platform this is.
#
# Note that `cargo build --release -p coral-app` does NOT: the frontend is embedded by the
# Tauri CLI's build step, and a plain cargo release build produces a binary that starts, opens
# a window, and never loads a page. Verified both ways, with the same freshly built ui/dist in
# place.
#
# The bundle targets come from tauri.conf.json and are filtered by the Tauri CLI to the ones
# this platform can actually produce, so one recipe covers all three. `build-mac` and
# `build-windows` exist for the cross-details that recipe cannot express.
build *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    # The two halves need nothing from each other, and both are slow from cold: `npm ci`
    # fetches the whole dependency tree while cargo compiles the CLI.
    ( cd ui && npm ci ) &
    npm_pid=$!
    # The CLI ships beside the application, so it has to exist before the bundle is assembled.
    cargo build --release -p coral-cli
    wait $npm_pid
    cd crates/coral-app && cargo tauri build {{ARGS}}

# A universal macOS build, which is what a .dmg should carry: an Intel-only bundle runs under
# Rosetta on Apple silicon and a native-only one will not start on an Intel Mac at all. Both
# targets have to be installed — `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.
build-mac:
    just build --target universal-apple-darwin

# Windows produces an NSIS installer from the same configuration. WebView2 is assumed present:
# it ships with Windows 11 and with every supported Windows 10, and bundling the bootstrapper
# would add a download to an installer that does not need one on any current system.
build-windows:
    just build

# Every platform's bundles, one machine each. There is no cross-compiling here: a Tauri bundle
# links the platform's own webview, so each has to be built where it runs. This is what the
# release workflow does on its three runners.
build-all:
    @echo 'Bundles are built per platform; run `just build` on Linux, macOS and Windows.'
    @echo 'CI does this on three runners: see .github/workflows/release.yml.'

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

# Open the benchmark clone. Exists so IDE run configurations need no shell expansion.
open-kernel:
    cargo run --release -p coral-cli -- --repo '{{kernel}}' --json open

licenses:
    cargo about generate about.hbs > THIRD_PARTY_LICENSES.md

# Fails if a dependency was added without regenerating the licence file. Part of the gate
# because it needs no network and this drifted unnoticed for a whole milestone: `licenses` and
# `deny` both failed on a dependency's licence and nothing ran either of them.
licenses-drift: licenses
    git diff --exit-code -- THIRD_PARTY_LICENSES.md

deny:
    cargo deny check
