#!/usr/bin/env bash
#
# fixtures.sh <dir> — the two repositories the real-window tour opens.
#
#   <dir>/work     a small history with branches, a tag, a stash, and a working tree with one
#                  hunk staged, one not, and a new file
#   <dir>/merging  the same history stopped in a merge with a conflict in src/lanes.ts
#
# Every commit is a fixture identity. Nothing here reads the user's git configuration.
set -euo pipefail
D=$1
rm -rf "$D/work" "$D/merging"
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

sed -i 's/    open.delete(lane);/&\n    if (open.size === 0) break;/' src/lanes.ts
git add src/lanes.ts
sed -i 's/    ctx.lineTo(laneX(lane), rowY(window.last));/    ctx.lineTo(laneX(lane), rowY(window.last) + half);/' src/lanes.ts
printf 'untracked\n' > docs/todo.md
