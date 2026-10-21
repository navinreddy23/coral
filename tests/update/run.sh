#!/usr/bin/env bash
#
# An installed Coral updating itself, from end to end, with nothing leaving this machine.
#
# Two AppImages are built a version apart, signed with a key made for this run and thrown away
# after it: never the release key. The newer one is published the way a release publishes it,
# through .github/release/updater.sh, from a server on 127.0.0.1. The older one runs on a
# private X display, is asked from the palette to check and then to install, and:
#
# - the AppImage on disk is the newer one afterwards, byte for byte;
# - the window comes back, from a new process, and that process is the newer version;
# - a release signed with another key is refused, and the AppImage is left as it was.
#
# Slow: the application is built twice, into target/update-test so the release build is not
# disturbed. Needs Xephyr, ImageMagick, python3 and FUSE for the AppImage.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="$root/target/update-test"
OLD=9.9.0
NEW=9.9.1

for tool in Xephyr import python3 cargo; do
    command -v "$tool" >/dev/null || { echo "update-test needs $tool" >&2; exit 1; }
done

venv="$root/target/window-real/venv"
if [ ! -x "$venv/bin/python" ]; then
    python3 -m venv "$venv"
    "$venv/bin/pip" install --quiet python-xlib
fi

work="$(mktemp -d)"
pids=()
# The restarted window is no child of this script, and an AppImage runs from a mount whose
# name says nothing about this run, so a window of this run's is known by the home it was
# given. Anything else called Coral, such as the one the user is working in, is left alone.
stop_coral() {
    local p
    for p in $(pgrep -x coral-app; pgrep -f '^[^ ]*/Coral\.AppImage$'); do
        if tr '\0' '\n' < "/proc/$p/environ" 2>/dev/null | grep -qx "HOME=$work/home"; then
            kill "$p" 2>/dev/null || true
        fi
    done
}
cleanup() {
    stop_coral
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
    rm -rf "$work"
}
trap cleanup EXIT

port="$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')"

# A key for this run, and a second one to sign a release nobody should accept.
cargo tauri signer generate --ci -p test -w "$work/key" >/dev/null
cargo tauri signer generate --ci -p test -w "$work/stranger" >/dev/null

# The application as a release builds it, but at a made-up version, reading its update from
# here. Plain http is allowed for the loopback server only in this build.
build() {
    local version=$1 config
    config="$(python3 - "$version" "$(cat "$work/key.pub")" "$port" <<'PY'
import json, sys
version, pubkey, port = sys.argv[1:]
print(json.dumps({
    "version": version,
    "bundle": {"createUpdaterArtifacts": True},
    "plugins": {"updater": {
        "pubkey": pubkey,
        "endpoints": [f"http://127.0.0.1:{port}/latest.json"],
        "dangerousInsecureTransportProtocol": True,
    }},
}))
PY
)"
    (cd "$root/crates/coral-app" && CARGO_TARGET_DIR="$out/target" \
        TAURI_SIGNING_PRIVATE_KEY="$(cat "$work/key")" TAURI_SIGNING_PRIVATE_KEY_PASSWORD=test \
        cargo tauri build --bundles appimage --config "$config" >"$work/build-$version.log" 2>&1) \
        || { tail -30 "$work/build-$version.log"; exit 1; }
    cp "$out/target/release/bundle/appimage/Coral_${version}_amd64.AppImage" "$work/$version.AppImage"
    cp "$out/target/release/bundle/appimage/Coral_${version}_amd64.AppImage.sig" "$work/$version.AppImage.sig"
}
echo "building $OLD and $NEW; the first run compiles everything"
build "$OLD"
build "$NEW"

# The release, as the release job lays it out. Only the Linux x86-64 AppImage is the real
# thing; the rest are there because updater.sh will not describe a release with a platform
# missing.
release="$work/release"
mkdir -p "$release"
while read -r name; do
    case "$name" in *.sig) ;; *) echo "not this platform" > "$release/$name" ;; esac
done < <("$root/.github/release/manifest.sh" --list "$NEW")
for name in "$release"/*; do
    case "$name" in *.AppImage | *.deb | *.app.tar.gz | *-setup.exe) echo "unsigned" > "$name.sig" ;; esac
done
cp "$work/$NEW.AppImage" "$release/Coral_${NEW}_linux-x86_64.AppImage"
cp "$work/$NEW.AppImage.sig" "$release/Coral_${NEW}_linux-x86_64.AppImage.sig"
printf 'A made-up release for the update test.\n' > "$work/notes.md"
"$root/.github/release/updater.sh" "$NEW" "$release" "http://127.0.0.1:$port" "$work/notes.md"

python3 -m http.server "$port" --bind 127.0.0.1 --directory "$release" >"$work/server.log" 2>&1 &
pids+=($!)

n=90
while [ -e "/tmp/.X11-unix/X$n" ]; do n=$((n + 1)); done
DISPLAY_ID=":$n"
Xephyr "$DISPLAY_ID" -screen 1500x950 -ac -noreset >/dev/null 2>&1 &
pids+=($!)
for _ in $(seq 1 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
: > "$work/wm.log"
"$venv/bin/python" -I "$root/tests/window-real/wm.py" "$DISPLAY_ID" "$work/wm.log" &
pids+=($!)

bash "$root/tests/window-real/fixtures.sh" "$work" >/dev/null 2>&1
mkdir -p "$out/shots" "$work/bin"
rm -f "$out"/shots/*.png
printf '#!/bin/sh\nexit 255\n' > "$work/bin/ssh"
chmod +x "$work/bin/ssh"

# Installed where a user keeps an AppImage, and run from there.
launch() {
    local home="$work/home"
    mkdir -p "$home/.config/dev.coral.app"
    printf '{"tabs":[{"id":1,"path":"%s","group":null}],"groups":[],"active":1,"nextId":2}' \
        "$work/work" > "$home/.config/dev.coral.app/session.json"
    env -u WAYLAND_DISPLAY XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 DISPLAY="$DISPLAY_ID" \
        HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
        XDG_CACHE_HOME="$home/.cache" PATH="$work/bin:$PATH" \
        "$work/apps/Coral.AppImage" >>"$work/app.log" 2>&1 &
}

status=0

# A release signed with a key this build does not trust is refused, and nothing is replaced.
mkdir -p "$work/apps"
cp "$work/$OLD.AppImage" "$work/apps/Coral.AppImage"
cp "$release/latest.json" "$work/latest.trusted.json"
(cd "$root/crates/coral-app" && TAURI_SIGNING_PRIVATE_KEY="$(cat "$work/stranger")" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD=test \
    cargo tauri signer sign "$release/Coral_${NEW}_linux-x86_64.AppImage" >/dev/null 2>&1)
"$root/.github/release/updater.sh" "$NEW" "$release" "http://127.0.0.1:$port" "$work/notes.md" >/dev/null
launch
"$venv/bin/python" -I "$here/install.py" "$DISPLAY_ID" "$out/shots" refused || status=$?
if cmp -s "$work/apps/Coral.AppImage" "$work/$OLD.AppImage"; then
    echo "ok   the AppImage is untouched after a refused update"
else
    echo "FAIL the AppImage was replaced by a download that did not verify"; status=1
fi
stop_coral
sleep 2

# And with the release as it was signed.
cp "$work/$NEW.AppImage.sig" "$release/Coral_${NEW}_linux-x86_64.AppImage.sig"
cp "$work/latest.trusted.json" "$release/latest.json"
# The binary inside the newer AppImage, which the restarted window has to be running. Its
# version cannot be read off the window: About shows the crate's, which this test overrides
# only for the updater.
(cd "$work" && "./$NEW.AppImage" --appimage-extract usr/bin/coral-app >/dev/null)
launch
"$venv/bin/python" -I "$here/install.py" "$DISPLAY_ID" "$out/shots" installed \
    "$work/squashfs-root/usr/bin/coral-app" || status=$?
if cmp -s "$work/apps/Coral.AppImage" "$work/$NEW.AppImage"; then
    echo "ok   the AppImage on disk is $NEW, byte for byte"
else
    echo "FAIL the AppImage on disk is not $NEW"; status=1
fi
[ -x "$work/apps/Coral.AppImage" ] && echo "ok   and it is still executable" \
    || { echo "FAIL the new AppImage is not executable"; status=1; }
grep -q "GET /Coral_${NEW}_linux-x86_64.AppImage " "$work/server.log" \
    && echo "ok   the AppImage was downloaded from the release" \
    || { echo "FAIL the server never served the AppImage"; status=1; }

echo "photographs in $out/shots"
exit "$status"
