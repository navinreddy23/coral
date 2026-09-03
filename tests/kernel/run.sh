#!/usr/bin/env bash
# Kernel-scale scenarios. Uses only the `coral` CLI plus plain git for assertions, so it
# exercises exactly what the app exercises.
#
# Scenarios are added as the engine gains the operations they need; each one that is not yet
# implemented reports SKIP rather than silently passing.
set -euo pipefail

REPO="${CORAL_KERNEL_REPO:-$HOME/.cache/coral-bench/linux}"
# CI hardware is slower than a developer laptop; budgets are multiplied by this.
SCALE="${CORAL_BUDGET_SCALE:-1}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORAL="${CORAL_BIN:-$ROOT/target/release/coral}"

pass=0; fail=0; skip=0

ok()   { printf '  \033[32mPASS\033[0m %s\n' "$1"; pass=$((pass+1)); }
bad()  { printf '  \033[31mFAIL\033[0m %s\n' "$1"; fail=$((fail+1)); }
miss() { printf '  \033[33mSKIP\033[0m %s\n' "$1"; skip=$((skip+1)); }

budget() { echo $(( $1 * SCALE )); }

require_repo() {
    [ -d "$REPO/.git" ] || { echo "no kernel clone at $REPO; run 'just kernel-clone'" >&2; exit 1; }
    [ -x "$CORAL" ] || { echo "no release binary at $CORAL; run 'cargo build --release'" >&2; exit 1; }
}

# Scenario 1 — open and report. Budget: 300 ms to a usable RepoInfo.
scenario_open() {
    echo "1. open"
    local start elapsed limit
    start=$(date +%s%N)
    local out; out="$("$CORAL" --repo "$REPO" --json open)"
    elapsed=$(( ($(date +%s%N) - start) / 1000000 ))
    limit=$(budget 300)

    if [ "$(jq -r .ok <<<"$out")" = "true" ]; then ok "open returns ok"; else bad "open returned $(jq -c .error <<<"$out")"; fi
    if [ "$(jq -r .result.commitGraph <<<"$out")" = "true" ]; then
        ok "commit-graph present"
    else
        bad "commit-graph absent — benchmarks are not meaningful; run 'just kernel-clone'"
    fi
    if [ "$elapsed" -le "$limit" ]; then
        ok "open took ${elapsed}ms (budget ${limit}ms)"
    else
        bad "open took ${elapsed}ms, over budget ${limit}ms"
    fi
}

scenario_status()      { echo "2. status";              miss "needs coral status (M1)"; }
scenario_merge()       { echo "3. merge conflict";      miss "needs coral merge + conflicts (M2/M3)"; }
scenario_rebase()      { echo "4. rebase with stops";   miss "needs coral rebase (M2/M3)"; }
scenario_cherry_pick() { echo "5. cherry-pick/revert";  miss "needs coral cherry-pick (M2)"; }
scenario_branching()   { echo "6. branching";           miss "needs coral branch (M2)"; }
scenario_remotes()     { echo "7. remotes";             miss "needs coral fetch/push (M4)"; }
scenario_abort()       { echo "8. abort paths";         miss "needs coral op abort (M3)"; }

cleanup() {
    # Every scenario branch is prefixed bench/; the cache clone itself is left intact.
    git -C "$REPO" for-each-ref --format='%(refname:short)' 'refs/heads/bench/*' 2>/dev/null \
        | while read -r b; do git -C "$REPO" branch -D "$b" >/dev/null 2>&1 || true; done
}
trap cleanup EXIT

require_repo
echo "kernel scenarios against $REPO (budget scale ${SCALE}x)"
scenario_open
scenario_status
scenario_merge
scenario_rebase
scenario_cherry_pick
scenario_branching
scenario_remotes
scenario_abort

printf '\n%d passed, %d failed, %d not yet implemented\n' "$pass" "$fail" "$skip"
[ "$fail" -eq 0 ]
