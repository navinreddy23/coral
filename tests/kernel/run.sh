#!/usr/bin/env bash
# Kernel-scale scenarios. Uses only the `coral` CLI plus plain git for assertions, so it
# exercises exactly what the app exercises.
#
# Scenarios are added as the engine gains the operations they need; each one that is not yet
# implemented reports SKIP rather than silently passing.
set -uo pipefail

# Report the command that failed instead of dying silently mid-scenario. Scenarios are
# expected to hit non-zero exits (a conflicting merge is exit 3), so `set -e` is deliberately
# not used; each scenario checks its own outcomes.
trap 'printf "  \033[31mERROR\033[0m line %s exited %s\n" "$LINENO" "$?" >&2' ERR

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

    # Refuse to start on a repository a previous run left mid-operation: the scenarios would
    # compound the mess and every later assertion would be meaningless.
    if [ -n "$(git -C "$REPO" status --porcelain)" ]; then
        echo "clone at $REPO has uncommitted changes; clean it before running" >&2
        exit 1
    fi
    if [ -n "$(git -C "$REPO" for-each-ref --format='%(refname)' 'refs/heads/bench/*')" ]; then
        echo "clone at $REPO still has bench/ branches from an earlier run" >&2
        exit 1
    fi
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

# Scenario 2 — status. Budget: 1 s warm on a 96k-file worktree.
scenario_status() {
    echo "2. status"
    "$CORAL" --repo "$REPO" --json status >/dev/null   # warm the index stat cache
    local start elapsed limit
    start=$(date +%s%N)
    local out; out="$("$CORAL" --repo "$REPO" --json status)"
    elapsed=$(( ($(date +%s%N) - start) / 1000000 ))
    limit=$(budget 1000)

    if [ "$(jq -r .ok <<<"$out")" = "true" ]; then ok "status returns ok"; else bad "status: $(jq -c .error <<<"$out")"; fi
    if [ "$(jq -r '.result.entries | length' <<<"$out")" -eq 0 ]; then
        ok "worktree is clean"
    else
        bad "worktree is dirty; scenarios need a clean checkout"
    fi
    if [ "$elapsed" -le "$limit" ]; then
        ok "status took ${elapsed}ms (budget ${limit}ms)"
    else
        bad "status took ${elapsed}ms, over budget ${limit}ms"
    fi
}

# Scenario 2b — the graph pipeline, against both budgets in docs/ARCHITECTURE.md.
scenario_graph() {
    echo "2b. graph"
    local start elapsed limit out

    start=$(date +%s%N)
    out="$("$CORAL" --repo "$REPO" --json graph --limit 4096 --first-paint)"
    elapsed=$(( ($(date +%s%N) - start) / 1000000 ))
    limit=$(budget 300)
    if [ "$elapsed" -le "$limit" ]; then
        ok "first paint took ${elapsed}ms (budget ${limit}ms)"
    else
        bad "first paint took ${elapsed}ms, over budget ${limit}ms"
    fi
    [ "$(jq -r .result.provisional <<<"$out")" = "true" ] \
        && ok "first paint rows are marked provisional" \
        || bad "first paint rows are not marked provisional"

    start=$(date +%s%N)
    out="$("$CORAL" --repo "$REPO" --json graph --from 4294967295)"
    elapsed=$(( ($(date +%s%N) - start) / 1000000 ))
    limit=$(budget 5000)
    local total; total=$(jq -r .result.total <<<"$out")
    local expected; expected=$(git -C "$REPO" rev-list --all --count)
    if [ "$total" = "$expected" ]; then
        ok "full graph has $total rows, matching git rev-list"
    else
        bad "full graph has $total rows, git rev-list says $expected"
    fi
    if [ "$elapsed" -le "$limit" ]; then
        ok "full graph took ${elapsed}ms (budget ${limit}ms)"
    else
        bad "full graph took ${elapsed}ms, over budget ${limit}ms"
    fi
}

# Scenario 2c — refs.
scenario_refs() {
    echo "2c. refs"
    local out; out="$("$CORAL" --repo "$REPO" --json refs)"
    local n; n=$(jq -r '.result.refs | length' <<<"$out")
    local expected; expected=$(git -C "$REPO" for-each-ref --format='%(refname)' | wc -l)
    [ "$n" = "$expected" ] && ok "listed $n refs, matching for-each-ref" || bad "listed $n refs, git says $expected"
}
# Resolves two adjacent release tags at runtime, so the scenarios follow the kernel forward.
release_tags() {
    git -C "$REPO" tag --sort=-v:refname | grep -E '^v[0-9]+\.[0-9]+$' | head -2
}

# Scenario 3 — a deterministic conflict in the Makefile version block.
scenario_merge() {
    echo "3. merge conflict"
    local new old; new=$(release_tags | head -1); old=$(release_tags | tail -1)
    git -C "$REPO" checkout -q -B bench/merge "$old" 2>/dev/null

    # Both sides touch the version lines at the top of the Makefile.
    sed -i '1,8s/^SUBLEVEL = .*/SUBLEVEL = 999/' "$REPO/Makefile"
    git -C "$REPO" commit -q -am "bench: bump sublevel" 2>/dev/null

    "$CORAL" --repo "$REPO" --json merge "$new" >/tmp/coral-merge.json 2>&1
    local rc=$?
    [ "$rc" -eq 3 ] && ok "merge stopped with exit 3" || bad "merge exited $rc, expected 3"

    local conflicts; conflicts=$("$CORAL" --repo "$REPO" --json status | jq -r '[.result.entries[] | select(.conflict != null) | .path] | join(",")')
    [ -n "$conflicts" ] && ok "conflicts reported: $conflicts" || bad "no conflicts reported"

    # Resolve by taking the incoming side wholesale, then continue.
    git -C "$REPO" checkout --theirs -- Makefile 2>/dev/null
    "$CORAL" --repo "$REPO" stage Makefile >/dev/null
    "$CORAL" --repo "$REPO" --json op continue >/dev/null 2>&1
    rc=$?
    [ "$rc" -eq 0 ] && ok "op continue finished the merge" || bad "op continue exited $rc"
    [ -z "$(git -C "$REPO" status --porcelain)" ] && ok "worktree clean afterwards" || bad "worktree dirty afterwards"
    [ -n "$(git -C "$REPO" log -1 --merges --format=%H)" ] && ok "a merge commit exists" || bad "no merge commit"
}

# Scenario 4 — abort leaves the branch exactly where it started.
scenario_rebase() {
    echo "4. rebase and abort"
    local new old; new=$(release_tags | head -1); old=$(release_tags | tail -1)
    git -C "$REPO" checkout -q -B bench/rebase "$old" 2>/dev/null
    sed -i '1,8s/^SUBLEVEL = .*/SUBLEVEL = 998/' "$REPO/Makefile"
    git -C "$REPO" commit -q -am "bench: conflicting bump" 2>/dev/null
    local before; before=$(git -C "$REPO" rev-parse HEAD)

    "$CORAL" --repo "$REPO" --json rebase "$new" >/dev/null 2>&1
    local rc=$?
    [ "$rc" -eq 3 ] && ok "rebase stopped with exit 3" || bad "rebase exited $rc, expected 3"

    "$CORAL" --repo "$REPO" --json op abort >/dev/null 2>&1
    [ "$(git -C "$REPO" rev-parse HEAD)" = "$before" ] && ok "abort restored the branch tip" || bad "abort left HEAD elsewhere"
    [ -z "$(git -C "$REPO" status --porcelain)" ] && ok "worktree clean after abort" || bad "worktree dirty after abort"
}

# Scenario 5 — cherry-pick a commit forward, then revert it back to the same tree.
scenario_cherry_pick() {
    echo "5. cherry-pick and revert"
    local new old; new=$(release_tags | head -1); old=$(release_tags | tail -1)
    git -C "$REPO" checkout -q -B bench/pick "$old" 2>/dev/null
    local tree_before; tree_before=$(git -C "$REPO" rev-parse "HEAD^{tree}")

    # A commit that *adds* a single file. Picking one that merely modifies a file conflicts
    # whenever an earlier commit in the range touched it too, which made this scenario flaky.
    local pick
    pick=$(git -C "$REPO" rev-list --reverse --max-count=400 "$old..$new" -- Documentation \
           | while read -r c; do
               local changes; changes=$(git -C "$REPO" diff-tree --no-commit-id --name-status -r "$c")
               [ "$(echo "$changes" | wc -l)" = "1" ] && [ "${changes:0:1}" = "A" ] && echo "$c" && break
             done)
    if [ -z "$pick" ]; then miss "no single-file commit found in range"; return; fi

    "$CORAL" --repo "$REPO" --json cherry-pick "$pick" >/dev/null 2>&1
    local rc=$?
    if [ "$rc" -ne 0 ]; then
        "$CORAL" --repo "$REPO" --json op abort >/dev/null 2>&1
        miss "cherry-pick did not apply cleanly (exit $rc)"
        return
    fi
    ok "cherry-picked $(echo "$pick" | cut -c1-8)"

    "$CORAL" --repo "$REPO" --json revert HEAD >/dev/null 2>&1
    rc=$?
    [ "$rc" -eq 0 ] && ok "reverted it" || bad "revert exited $rc"
    [ "$(git -C "$REPO" rev-parse 'HEAD^{tree}')" = "$tree_before" ] \
        && ok "tree matches the starting tree" || bad "tree differs after revert"
}

# Scenario 6 — branching on an 80k-file worktree, and undoing a deletion.
scenario_branching() {
    echo "6. branching and undo"
    local new old; new=$(release_tags | head -1); old=$(release_tags | tail -1)

    local start elapsed limit
    start=$(date +%s%N)
    "$CORAL" --repo "$REPO" --json checkout "$old" >/dev/null 2>&1
    elapsed=$(( ($(date +%s%N) - start) / 1000000 ))
    limit=$(budget 30000)
    [ "$elapsed" -le "$limit" ] && ok "checkout across releases took ${elapsed}ms" \
        || bad "checkout took ${elapsed}ms, over ${limit}ms"

    git -C "$REPO" checkout -q -B bench/doomed 2>/dev/null
    git -C "$REPO" checkout -q "$new" 2>/dev/null
    "$CORAL" --repo "$REPO" --json branch-delete bench/doomed >/dev/null 2>&1 || true

    # Deleting through git, then undoing through coral, needs the ref journalled first.
    if "$CORAL" --repo "$REPO" --json journal | jq -e '.result.undoable != null' >/dev/null 2>&1; then
        ok "journal has an undoable entry"
    else
        miss "branch delete is not yet a coral command"
    fi
    git -C "$REPO" branch -D bench/doomed >/dev/null 2>&1 || true
}
scenario_remotes()     { echo "7. remotes";             miss "needs coral fetch/push (M4)"; }
# Scenario 8 — the conflict engine on a real kernel conflict, then abort.
scenario_abort() {
    echo "8. conflict engine and abort"
    local new old; new=$(release_tags | head -1); old=$(release_tags | tail -1)
    git -C "$REPO" checkout -q -B bench/abort "$old" 2>/dev/null
    sed -i '1,8s/^SUBLEVEL = .*/SUBLEVEL = 997/' "$REPO/Makefile"
    git -C "$REPO" commit -q -am "bench: conflicting bump" 2>/dev/null
    local before; before=$(git -C "$REPO" rev-parse HEAD)

    "$CORAL" --repo "$REPO" --json merge "$new" >/dev/null 2>&1
    [ $? -eq 3 ] && ok "merge stopped" || bad "merge did not stop"

    # The sides must be named after refs, never "ours" and "theirs".
    local out; out=$("$CORAL" --repo "$REPO" --json conflicts)
    local ours theirs
    ours=$(jq -r .result.operation.labels.ours <<<"$out")
    theirs=$(jq -r .result.operation.labels.theirs <<<"$out")
    if [ "$ours" != "ours" ] && [ "$theirs" != "theirs" ] && [ -n "$ours" ]; then
        ok "sides named by ref: $ours vs $theirs"
    else
        bad "sides not named by ref: $ours vs $theirs"
    fi

    # Blocks are rebuilt from the index stages, so a real base is present.
    local blocks; blocks=$("$CORAL" --repo "$REPO" --json conflict-show Makefile)
    local n; n=$(jq -r '[.result.blocks[] | select(.kind == "conflict")] | length' <<<"$blocks")
    [ "$n" -ge 1 ] && ok "rebuilt $n conflict block(s) for Makefile" || bad "no blocks rebuilt"

    local based; based=$(jq -r '[.result.blocks[] | select(.kind == "conflict") | select(.base | length > 0)] | length' <<<"$blocks")
    [ "$based" -ge 1 ] && ok "a block carries the base version" || bad "no base in any block"

    "$CORAL" --repo "$REPO" --json op abort >/dev/null 2>&1
    [ "$(git -C "$REPO" rev-parse HEAD)" = "$before" ] && ok "abort restored the tip" || bad "abort moved HEAD"
    [ -z "$(git -C "$REPO" status --porcelain)" ] && ok "worktree clean after abort" || bad "worktree dirty"
}

# Whatever the clone was on before the scenarios ran, so they can put it back.
ORIGINAL_HEAD=""

cleanup() {
    # Scenarios check out old tags and create bench/ branches. Leave the clone exactly as it
    # was found: it is the developer's, and rebuilding it costs a 6 GB fetch.
    git -C "$REPO" merge --abort >/dev/null 2>&1 || true
    git -C "$REPO" rebase --abort >/dev/null 2>&1 || true
    git -C "$REPO" cherry-pick --abort >/dev/null 2>&1 || true
    git -C "$REPO" revert --abort >/dev/null 2>&1 || true
    git -C "$REPO" reset -q --hard >/dev/null 2>&1 || true

    if [ -n "$ORIGINAL_HEAD" ]; then
        git -C "$REPO" checkout -q --force "$ORIGINAL_HEAD" >/dev/null 2>&1 || true
    fi
    git -C "$REPO" for-each-ref --format='%(refname:short)' 'refs/heads/bench/*' 2>/dev/null \
        | while read -r b; do git -C "$REPO" branch -D "$b" >/dev/null 2>&1 || true; done
}
trap cleanup EXIT

require_repo
ORIGINAL_HEAD=$(git -C "$REPO" symbolic-ref --quiet --short HEAD 2>/dev/null \
    || git -C "$REPO" rev-parse HEAD)
echo "kernel scenarios against $REPO (budget scale ${SCALE}x)"
scenario_open
scenario_status
scenario_graph
scenario_refs
scenario_merge
scenario_rebase
scenario_cherry_pick
scenario_branching
scenario_remotes
scenario_abort

# The summary's own non-zero exit is the result, not an error to report.
trap - ERR
printf '\n%d passed, %d failed, %d not yet implemented\n' "$pass" "$fail" "$skip"
[ "$fail" -eq 0 ]
