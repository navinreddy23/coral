#!/usr/bin/env bash
#
# fixtures.sh <dir> — the repositories the real-window tour opens.
#
#   <dir>/work      a small history with branches, a tag, a stash, and a working tree with one
#                   hunk staged, one not, and a new file
#   <dir>/merging   the same history stopped in a merge with a conflict in src/lanes.ts
#   <dir>/shallow   the first three commits of work, cloned shallow
#   <dir>/branches  a team's history, for the graph: branches merged one after another with
#                   eight open at once, a branch off a branch, main merged into a feature, a
#                   branch merged twice, an octopus, a criss-cross, an orphan gh-pages beside
#                   it all, an unrelated history merged in, tags, and branches never merged
#
# Every commit is a fixture identity. Nothing here reads the user's git configuration.
set -euo pipefail
D=$1
rm -rf "$D/work" "$D/merging" "$D/branches"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

# Every commit a day after the last, from a fixed start, so the object ids, the ages and with
# them the photographs come out the same on every run, and a change between two runs is the
# window's rather than the clock's.
when=$(date -d 2025-01-06T09:00:00Z +%s)
commit() { # name email message
    when=$((when + 86400))
    GIT_AUTHOR_NAME="$1" GIT_AUTHOR_EMAIL="$2" GIT_COMMITTER_NAME="$1" GIT_COMMITTER_EMAIL="$2" \
        GIT_AUTHOR_DATE="@$when +0000" GIT_COMMITTER_DATE="@$when +0000" \
        git -c commit.gpgsign=false commit -qam "$3"
}
ADA=("Ada Lovelace" ada@fixture.invalid)
GRACE=("Grace Hopper" grace@fixture.invalid)
ALAN=("Alan Turing" alan@fixture.invalid)

git init -q -b main "$D/work"
cd "$D/work"
git config user.name "Ada Lovelace"
git config user.email ada@fixture.invalid
mkdir -p src docs
cat > src/lanes.ts <<'TS'
export function drawThroughLanes(ctx: Context, frame: Frame, window: Window): void {
  const open = new Map<number, number>();
  for (let row = window.first; row <= window.last; row++) {
    const lane = frame.lanes[row];
    if (lane === undefined) continue;
    open.delete(lane);
  }
  for (const [lane, fromY] of open) {
    ctx.moveTo(laneX(lane), fromY);
    ctx.lineTo(laneX(lane), rowY(window.last));
  }
  ctx.stroke();
}
TS
printf '# Notes\n\nHow the graph is drawn.\n' > docs/notes.md
git add -A
commit "${ADA[@]}" "core: draw the lanes that cross the window"
printf '\nA lane stays open until its parent row.\n' >> docs/notes.md
commit "${GRACE[@]}" "docs: say when a lane closes"
sed -i 's/  const open = new Map<number, number>();/&\n  const entering = frame.open[window.first - frame.startRow] ?? 0;/' src/lanes.ts
commit "${ADA[@]}" "graph: seed the open lanes from the frame's mask"
git tag v0.1.0
for i in 1 2 3; do
    printf 'Step %s of the walk.\n' "$i" >> docs/notes.md
    commit "${GRACE[@]}" "docs: walk through step $i"
done

git checkout -q -b feature/lanes
sed -i 's/    ctx.moveTo(laneX(lane), fromY);/    ctx.strokeStyle = laneColour(lane, colours);\n&/' src/lanes.ts
commit "${ALAN[@]}" "ui: colour each lane from the palette"
printf '// Lanes are drawn before nodes.\n' >> src/lanes.ts
commit "${ALAN[@]}" "ui: draw lanes under the nodes"

git checkout -q main
git checkout -q -b review/lanes
sed -i 's/    if (lane === undefined) continue;/    if (lane === undefined || lane === NO_LANE) continue;/' src/lanes.ts
commit "${ALAN[@]}" "graph: skip rows with no lane"

git checkout -q main
sed -i 's/  ctx.stroke();/&\n  ctx.closePath();/' src/lanes.ts
commit "${ADA[@]}" "graph: close the path after stroking"
printf 'A thought for later.\n' >> docs/notes.md
GIT_COMMITTER_DATE="@$((when + 3600)) +0000" git stash push -q -m "notes, half written"

git clone -q "$D/work" "$D/merging"
(
    cd "$D/merging"
    git config user.name "Ada Lovelace"
    git config user.email ada@fixture.invalid
    git branch -q review/lanes origin/review/lanes
    sed -i 's/    if (lane === undefined) continue;/    if (lane === undefined) break;/' src/lanes.ts
    commit "${ADA[@]}" "graph: stop at the first row with no lane"
    git -c commit.gpgsign=false merge -q review/lanes >/dev/null 2>&1 || true
)

# A third tab, cut off three commits down, for the status bar's "shallow clone".
git clone -q --depth 3 --single-branch "file://$D/work" "$D/shallow"

sed -i 's/    open.delete(lane);/&\n    if (open.size === 0) break;/' src/lanes.ts
git add src/lanes.ts
sed -i 's/    ctx.lineTo(laneX(lane), rowY(window.last));/    ctx.lineTo(laneX(lane), rowY(window.last) + half);/' src/lanes.ts
printf 'untracked\n' > docs/todo.md

# The graph's own fixture, in a subshell so the working tree above is left as it is. Each
# branch writes its own file, so no merge conflicts; commits are an hour apart, so the order
# is the one a team's history has rather than one decided by equal timestamps.
(
    git init -q -b main "$D/branches"
    cd "$D/branches"
    git config user.name "Ada Lovelace"
    git config user.email ada@fixture.invalid
    KATHERINE=("Katherine Johnson" katherine@fixture.invalid)
    MARGARET=("Margaret Hamilton" margaret@fixture.invalid)
    EDSGER=("Edsger Dijkstra" edsger@fixture.invalid)
    PEOPLE=(ADA GRACE ALAN KATHERINE MARGARET EDSGER)
    turn=0
    at() { # command...
        when=$((when + 3600))
        GIT_AUTHOR_DATE="@$when +0000" GIT_COMMITTER_DATE="@$when +0000" "$@"
    }
    as() { # person command...
        local -n who=$1
        shift
        GIT_AUTHOR_NAME="${who[0]}" GIT_AUTHOR_EMAIL="${who[1]}" \
            GIT_COMMITTER_NAME="${who[0]}" GIT_COMMITTER_EMAIL="${who[1]}" at "$@"
    }
    work() { # message — a commit on the branch checked out, by the next person in turn
        local file
        file="notes/$(git branch --show-current | tr / -).md"
        mkdir -p notes
        printf '%s\n' "$1" >> "$file"
        git add "$file"
        turn=$(((turn + 1) % ${#PEOPLE[@]}))
        as "${PEOPLE[$turn]}" git -c commit.gpgsign=false commit -qm "$1"
    }
    merge() { # message branch...
        local message=$1
        shift
        as ADA git -c commit.gpgsign=false merge -q --no-ff -m "$message" "$@"
    }
    on() { git checkout -q "$@"; }

    work "Start the project"
    work "Add a readme"
    git tag v0.1.0

    # An orphan for the website, which shares nothing with main.
    on --orphan gh-pages
    git rm -rq --cached . && rm -rf notes
    work "pages: first site"
    on main
    work "Set up the build"

    # Six branches at once off the same commit, merged one after another with main moving on
    # between, so the merge region is as wide as a busy week.
    for name in login search theme export billing audit; do
        on -b "feature/$name" main
        work "$name: sketch it"
    done
    for name in login search theme export billing audit; do
        on "feature/$name"
        work "$name: make it work"
    done
    on gh-pages && work "pages: add the guide"
    on main && work "Fix the build on Windows"
    merge "Merge feature/login" feature/login
    on feature/search && work "search: rank by recency"
    on main && merge "Merge feature/export" feature/export
    on feature/theme && work "theme: dark mode"
    on main && merge "Merge feature/theme" feature/theme

    # main into a feature, and the feature back later.
    on feature/search && merge "Merge main into feature/search" main
    work "search: page the results"
    on feature/billing && work "billing: invoices"

    # A branch off a branch, merged into it before it goes in itself.
    on -b feature/billing-tax feature/billing
    work "tax: rates table"
    work "tax: rounding"
    on feature/billing && work "billing: receipts"
    merge "Merge feature/billing-tax into feature/billing" feature/billing-tax
    on main && merge "Merge feature/search" feature/search
    merge "Merge feature/billing" feature/billing

    # A branch merged twice: in once, worked on, and in again.
    on main && work "Bump version"
    git tag -a -m "Release 1.0" v1.0
    on feature/theme && work "theme: high contrast"
    on main && merge "Merge feature/theme again" feature/theme
    on feature/audit && work "audit: export the log"
    on main && merge "Merge feature/audit" feature/audit

    # Three small fixes in one octopus.
    for n in 1 2 3; do
        on -b "fix/typo-$n" main
        work "fix typo $n"
    done
    on main && merge "Merge the typo fixes" fix/typo-1 fix/typo-2 fix/typo-3

    # A criss-cross: two long-lived lines that each merged the other.
    on -b release/1.x main && work "1.x: backport the date fix"
    on -b develop main && work "develop: new settings page"
    on release/1.x && merge "Merge develop into release/1.x" develop
    on develop && as ADA git -c commit.gpgsign=false merge -q --no-ff \
        -m "Merge release/1.x into develop" release/1.x~1
    on main && merge "Merge release/1.x" release/1.x
    merge "Merge develop" develop

    # Someone else's history, imported whole.
    on --orphan vendor/parser
    git rm -rq --cached . && rm -rf notes
    work "parser: import upstream"
    work "parser: upstream fixes"
    on main
    as ADA git -c commit.gpgsign=false merge -q --no-ff --allow-unrelated-histories \
        -m "Import the parser" vendor/parser
    on gh-pages && work "pages: release notes for 1.0"

    # Branches nobody has merged, one of them off a commit well down main.
    on -b wip/offline v0.1.0 && work "offline: cache the session"
    work "offline: sync on reconnect"
    on -b spike/graph main && work "spike: try a new layout"
    on main && work "Update the changelog"
    git tag -a -m "Release 1.1" v1.1
    work "Start 1.2"
    on wip/offline && work "offline: conflict banner"
    on main
)

