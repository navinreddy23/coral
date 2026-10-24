#!/usr/bin/env bash
#
# The window talking to a code host, with nothing leaving this machine.
#
# A fake GitHub Enterprise or GitLab answers on loopback, and Coral runs in a D-Bus session of
# its own with a throwaway keyring in a home of its own, so the token typed in reaches neither
# the desktop's keyring nor any real host. Signed in, the window lists the pull requests and
# asks for each open one's checks, and:
#
# - every request carries the token;
# - the checks are asked for every open proposal and no other;
# - a review is read, commented on and approved, each in the host's own requests;
# - the menu on a proposal checks it out, fetched from the host by the name it keeps it under;
# - the sidebar is photographed into target/hosting-test for a person to look at.
#
# `run.sh [github|gitlab]` picks the host, GitHub when none is named. `run.sh <host> --hold
# [seconds]` sets the same up and leaves it open instead.
#
# Needs Xephyr, ImageMagick, dbus-run-session and gnome-keyring-daemon.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="$root/target/hosting-test"
binary="$root/target/release/coral-app"
venv="$root/target/window-real/venv"

for tool in Xephyr import dbus-run-session gnome-keyring-daemon fusermount; do
    command -v "$tool" >/dev/null || { echo "hosting-test needs $tool" >&2; exit 1; }
done
[ -x "$binary" ] || { echo "no $binary; run just app first" >&2; exit 1; }
if [ ! -x "$venv/bin/python" ]; then
    python3 -m venv "$venv"
    "$venv/bin/pip" install --quiet python-xlib
fi

provider="${1:-github}"
case "$provider" in github | gitlab) shift || true ;; *) provider=github ;; esac
work="$(mktemp -d)"
pids=()
cleanup() {
    # The session's own daemons are found by the runtime directory they were given, so nothing
    # of the desktop's is touched.
    local p
    for p in $(pgrep -u "$(id -u)"); do
        if { tr '\0' '\n' < "/proc/$p/environ"; } 2>/dev/null | grep -qx "XDG_RUNTIME_DIR=$work/run"; then
            kill "$p" 2>/dev/null || true
        fi
    done
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
    sleep 1
    # The portal and gvfs mount FUSE filesystems there, which have to be let go to remove it.
    for m in gvfs doc; do fusermount -u "$work/run/$m" 2>/dev/null || true; done
    rm -rf "$work"
}
trap cleanup EXIT

port="$(python3 -c 'import socket; s = socket.socket(socket.AF_INET6); s.bind(("::1", 0)); print(s.getsockname()[1])')"
: > "$work/requests.log"
python3 "$here/server.py" "$port" "$work/requests.log" "$work/git" &
pids+=($!)

repo="$work/repo"
GIT_CONFIG_GLOBAL=/dev/null git init -q -b main "$repo"
(cd "$repo" && echo x > a && git add a \
    && GIT_CONFIG_GLOBAL=/dev/null git -c user.name=Ada -c user.email=ada@fixture.invalid commit -qm start \
    && git remote add origin "http://$provider.localhost:$port/o/r.git")

# The host's copy, with proposal 12's tip kept only where the host keeps a fork's: under its
# own ref, on no branch, so checking it out has to fetch it by that name.
GIT_CONFIG_GLOBAL=/dev/null git clone -q --bare "$repo" "$work/git/o/r.git"
contribution="$work/contribution"
GIT_CONFIG_GLOBAL=/dev/null git clone -q "$work/git/o/r.git" "$contribution"
(cd "$contribution" && echo y > lanes && git add lanes \
    && GIT_CONFIG_GLOBAL=/dev/null git -c user.name=Grace -c user.email=grace@fixture.invalid \
        commit -qm "Speed up the lane walk")
if [ "$provider" = github ]; then proposal=refs/pull/12/head; branch=pr/12; else
    proposal=refs/merge-requests/12/head; branch=mr/12; fi
GIT_CONFIG_GLOBAL=/dev/null git -C "$work/git/o/r.git" fetch -q "$contribution" "HEAD:$proposal"
# Work of our own on a branch the host has never seen, to open a request from.
(cd "$repo" && git switch -q -c topic && echo note > NOTE.md && git add NOTE.md \
    && GIT_CONFIG_GLOBAL=/dev/null git -c user.name=Ada -c user.email=ada@fixture.invalid \
        commit -qm "Add a note")

home="$work/home"
mkdir -p "$home/.config/dev.coral.app" "$home/.local/share/keyrings" "$work/bin"
mkdir -m 700 "$work/run"
printf '{"tabs":[{"id":1,"path":"%s","group":null}],"groups":[],"active":1,"nextId":2}' "$repo" \
    > "$home/.config/dev.coral.app/session.json"
printf '#!/bin/sh\nexit 255\n' > "$work/bin/ssh"
chmod +x "$work/bin/ssh"

n=91
while [ -e "/tmp/.X11-unix/X$n" ]; do n=$((n + 1)); done
Xephyr ":$n" -screen 1500x950 -ac -noreset >/dev/null 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
: > "$work/wm.log"
"$venv/bin/python" -I "$root/tests/window-real/wm.py" ":$n" "$work/wm.log" &
pids+=($!)

# The window, its keyring and its bus, all private. The runtime directory is the test's too:
# the keyring daemon finds a running one through it, and the desktop's is in the real one.
HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
    XDG_CACHE_HOME="$home/.cache" XDG_RUNTIME_DIR="$work/run" \
    dbus-run-session -- bash -c '
        echo -n throwaway | gnome-keyring-daemon --unlock --components=secrets >/dev/null 2>&1
        exec env -u WAYLAND_DISPLAY XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 DISPLAY="$0" \
            PATH="$1:$PATH" "$2"' ":$n" "$work/bin" "$binary" >"$work/app.log" 2>&1 &
pids+=($!)

if [ "${1:-}" = "--hold" ]; then
    echo "holding on DISPLAY=:$n, host on $provider.localhost:$port, for ${2:-600}s"
    sleep "${2:-600}"
    exit 0
fi

out="$out/$provider"
mkdir -p "$out"
rm -f "$out"/*.png
status=0
# The keyring the daemon made empty when it started, so that storing the token can be seen
# to change it.
keyring="$home/.local/share/keyrings/login.keyring"
for _ in $(seq 1 50); do [ -s "$keyring" ] && break; sleep 0.1; done
before="$(sha256sum "$keyring" 2>/dev/null || true)"
"$venv/bin/python" -I "$here/drive.py" ":$n" "$out" "$provider" || status=1

check() {
    if eval "$1"; then echo "ok   $2"; else echo "FAIL $2"; status=1; fi
}
log="$work/requests.log"
if [ "$provider" = github ]; then
    check "grep -q '/pulls?' '$log'" "the proposals were asked for"
    check "! grep -v 'auth=Bearer dumm' '$log' | grep -q ." "every request carried the token"
    # The fourth is the request opened from topic, whose head the fake host calls d….
    for sha in aaaaaaaa bbbbbbbb cccccccc dddddddd; do
        check "grep -q 'commits/$sha.*/check-runs' '$log'" "the checks on ${sha:0:1}… were asked for"
    done
    check "! grep -o 'commits/[0-9a-f]*/check-runs' '$log' | grep -vqE 'commits/(a{40}|b{40}|c{40}|d{40})/'" \
        "and on nothing else"
    check "grep -q 'issues/12/comments?' '$log'" "the review of #12 was read"
    check "grep -q 'POST /api/v3/repos/o/r/issues/12/comments .*Added a test' '$log'" "a comment was left on it"
    check "grep -q 'POST /api/v3/repos/o/r/pulls/12/reviews .*APPROVE' '$log'" "and it was approved, as a review"
    check "grep -q 'POST /api/v3/repos/o/r/pulls .*\"head\": \"topic\"' '$log'" "a pull request was opened from topic"
else
    check "grep -q '/merge_requests?' '$log'" "the merge requests were asked for"
    # GitLab takes the token in a header of its own rather than as a bearer.
    check "! grep -v 'private=dummy' '$log' | grep -q ." "every request carried the token"
    check "! grep -q 'auth=Bearer' '$log'" "in GitLab's header, not GitHub's"
    for iid in 13 12 11 10; do
        check "grep -q 'merge_requests/$iid/pipelines' '$log'" "the pipelines of !$iid were asked for"
    done
    check "! grep -o 'merge_requests/[0-9]*/pipelines' '$log' | grep -vqE 'merge_requests/1[0-3]/'" \
        "and of nothing else"
    check "grep -q 'merge_requests/12/notes?' '$log'" "the review of !12 was read"
    check "grep -q 'POST .*merge_requests/12/notes .*Added a test' '$log'" "a note was left on it"
    check "grep -q 'POST .*merge_requests/12/approve' '$log'" "and it was approved where GitLab counts approvals"
    check "grep -q 'POST /api/v4/projects/o%2Fr/merge_requests .*\"source_branch\": \"topic\"' '$log'" "a merge request was opened from topic"
fi
check "[ \"\$(git -C '$repo' symbolic-ref --short HEAD)\" = $branch ]" "#12 is checked out as $branch"
check "[ -e '$repo/lanes' ]" "with the request's own change in it"
check "[ \"\$(sha256sum '$keyring')\" != '$before' ]" "the token went to the test's own keyring"
check "GIT_CONFIG_GLOBAL=/dev/null git --git-dir '$work/git/o/r.git' rev-parse -q --verify refs/heads/topic >/dev/null" \
    "topic was pushed first, with the token the window signed in with"

echo "photographs in $out"
exit "$status"
