#!/usr/bin/env bash
# Photographs every surface of the window, in both themes, in a real browser.
#
# The frontend is served by Vite and answered by the fixture engine in `ui/src/ipc/preview.ts`,
# so this needs no repository, no Rust and no network — but it does need Chrome, which is why
# it is not part of `just check`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="${1:-$root/target/window-shots}"
chrome="${CORAL_CHROME:-/usr/bin/google-chrome}"

if [ ! -x "$chrome" ]; then
    echo "no browser at $chrome; set CORAL_CHROME to one" >&2
    exit 1
fi

cd "$root/ui"
[ -d node_modules ] && : || npm ci

rm -rf "$out"
mkdir -p "$out"

log="$(mktemp)"
npm run dev >"$log" 2>&1 &
server=$!
trap 'kill "$server" 2>/dev/null || true; rm -f "$log"' EXIT

# Vite is quick but not instant, and a sweep that starts against a closed port fails for a
# reason that has nothing to do with the window.
for _ in $(seq 1 60); do
    if curl -sf -o /dev/null http://localhost:5173/; then break; fi
    sleep 0.25
done
if ! curl -sf -o /dev/null http://localhost:5173/; then
    echo "the dev server never came up:" >&2
    cat "$log" >&2
    exit 1
fi

node "$here/sweep.mjs" "$out"
echo "look at them with: xdg-open $out"
