set shell := ["bash", "-euo", "pipefail", "-c"]

# cargo is not on the non-interactive PATH on every machine; see docs/DECISIONS.md.
export PATH := env_var("HOME") + "/.cargo/bin:" + env_var("PATH")
export RUST_BACKTRACE := "1"

kernel := env_var_or_default("CORAL_KERNEL_REPO", env_var("HOME") + "/.cache/coral-bench/linux")

default: check

# The gate. Everything must be green before a commit.
check: fmt-check lint test ui-check licenses-drift

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

# Fails if the generated bindings drift from the Rust types.
bindings-drift: test
    git diff --exit-code -- ui/src/ipc/types.ts

dev:
    cd crates/coral-app && cargo tauri dev

build:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="$HOME/.cargo/bin:$PATH"
    cd ui && npm ci && cd ..
    # The CLI ships beside the application, so it has to exist before the bundle is assembled.
    cargo build --release -p coral-cli
    cd crates/coral-app && cargo tauri build

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
