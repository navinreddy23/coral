set shell := ["bash", "-euo", "pipefail", "-c"]

# cargo is not on the non-interactive PATH on every machine; see docs/DECISIONS.md.
export PATH := env_var("HOME") + "/.cargo/bin:" + env_var("PATH")
export RUST_BACKTRACE := "1"

kernel := env_var_or_default("CORAL_KERNEL_REPO", env_var("HOME") + "/.cache/coral-bench/linux")

default: check

# The gate. Everything must be green before a commit.
check: fmt-check lint test ui-check

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
    cd ui && npm run check && npm run build

# Fails if the generated bindings drift from the Rust types.
bindings-drift: test
    git diff --exit-code -- ui/src/ipc/types.ts

dev:
    cd crates/coral-app && cargo tauri dev

build:
    cd ui && npm ci && npm run build
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

bench: kernel-clone
    CORAL_KERNEL_REPO='{{kernel}}' cargo run --release -p coral-cli -- bench all

licenses:
    cargo about generate about.hbs > THIRD_PARTY_LICENSES.md

deny:
    cargo deny check
