#!/usr/bin/env bash
#
# Does a release refuse to go out with a platform missing?
#
# 1.2.2 reached GitHub with the Linux x86-64 files and nothing else. The manifest that guards
# against that runs only on a tag, which is the worst place to find out it is wrong, so it is
# run here against release directories made up on the spot.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
MANIFEST="$ROOT/.github/release/manifest.sh"
DIR=$(mktemp -d)
trap 'rm -rf "$DIR"' EXIT

passed=0
ok() { printf 'ok   %s\n' "$1"; passed=$((passed + 1)); }
fail() { printf 'FAIL %s\n' "$1" >&2; exit 1; }

# What every release carries, built from the manifest's own list so the two cannot disagree.
complete() {
    local into=$1 version=$2
    mkdir -p "$into"
    "$MANIFEST" --list "$version" | while read -r name; do : > "$into/$name"; done
}

complete "$DIR/full" 1.2.3
"$MANIFEST" 1.2.3 "$DIR/full" >/dev/null || fail "a complete release is refused"
ok "a complete release is accepted"

complete "$DIR/linux-only" 1.2.3
rm "$DIR"/linux-only/*macos* "$DIR"/linux-only/*windows*
if out=$("$MANIFEST" 1.2.3 "$DIR/linux-only" 2>&1); then fail "a Linux-only release is accepted"; fi
grep -q 'Coral_1.2.3_macos-universal.dmg' <<<"$out" || fail "the missing .dmg is not named"
grep -q 'Coral_1.2.3_windows-aarch64-setup.exe' <<<"$out" || fail "the missing installer is not named"
ok "a release with only Linux in it is refused, naming what is missing"

complete "$DIR/no-cli" 1.2.3
rm "$DIR/no-cli/coral_1.2.3_windows-x86_64.exe"
"$MANIFEST" 1.2.3 "$DIR/no-cli" >/dev/null 2>&1 && fail "a release missing one CLI is accepted"
ok "a release missing a single command-line download is refused"

complete "$DIR/old" 1.2.2
"$MANIFEST" 1.2.3 "$DIR/old" >/dev/null 2>&1 && fail "last version's files pass for this one"
ok "files built for another version do not count"

complete "$DIR/extra" 1.2.3
: > "$DIR/extra/stray.txt"
"$MANIFEST" 1.2.3 "$DIR/extra" >/dev/null || fail "an extra file fails the release"
ok "an extra file is not a missing one"

# 1.2.0 is the last release that went out whole, so its file names are what a release is.
mkdir -p "$DIR/v1.2.0"
for name in Coral_1.2.0_linux-aarch64.AppImage Coral_1.2.0_linux-aarch64.deb \
        coral_1.2.0_linux-aarch64.tar.gz Coral_1.2.0_linux-x86_64.AppImage \
        Coral_1.2.0_linux-x86_64.deb coral_1.2.0_linux-x86_64.tar.gz \
        Coral_1.2.0_macos-universal.dmg coral_1.2.0_macos-universal.tar.gz \
        Coral_1.2.0_windows-aarch64-setup.exe coral_1.2.0_windows-aarch64.exe \
        Coral_1.2.0_windows-x86_64-setup.exe coral_1.2.0_windows-x86_64.exe SHA256SUMS.txt; do
    : > "$DIR/v1.2.0/$name"
done
# 1.2.0 shipped before updates could be installed in place. A release now carries the signature
# every in-place update is checked against, so 1.2.0's files are no longer a whole release.
if out=$("$MANIFEST" 1.2.0 "$DIR/v1.2.0" 2>&1); then fail "a release without signatures passes"; fi
grep -q 'Coral_1.2.0_linux-x86_64.AppImage.sig' <<<"$out" || fail "the missing signature is not named"
grep -q 'Coral_1.2.0_macos-universal.app.tar.gz' <<<"$out" || fail "the missing macOS update is not named"
ok "the files 1.2.0 shipped with are refused now, for want of update signatures"

# Publishing, against a stand-in gh that records what it was asked and answers as told.
PUBLISH="$ROOT/.github/release/publish.sh"
mkdir -p "$DIR/bin"
cat > "$DIR/bin/gh" <<'GH'
#!/usr/bin/env bash
echo "$*" >> "$GH_CALLS"
if [ "$1 $2" = "release view" ]; then
    [ -n "$GH_DRAFT" ] || exit 1
    echo "$GH_DRAFT"
fi
GH
chmod +x "$DIR/bin/gh"
: > "$DIR/notes.md"
publish() { # what `release view` answers: '' for no release, 'true' for a draft, 'false' for published
    : > "$DIR/calls"
    PATH="$DIR/bin:$PATH" GH_CALLS="$DIR/calls" GH_DRAFT=$1 \
        "$PUBLISH" v1.2.3 1.2.3 "$DIR/full" "$DIR/notes.md" >/dev/null 2>&1
}

publish '' || fail "publishing a new release failed"
grep -q '^release create v1.2.3 .*--draft --title Coral 1.2.3' "$DIR/calls" \
    || fail "a new tag was not made a draft named Coral 1.2.3: $(cat "$DIR/calls")"
ok "a new tag becomes a draft named Coral <version>"

publish true || fail "re-running onto a draft failed"
grep -q '^release upload v1.2.3 .*--clobber' "$DIR/calls" || fail "a draft was not refilled"
ok "a draft from an earlier run of the tag is refilled"

publish false && fail "files were uploaded into a published release"
grep -q '^release upload' "$DIR/calls" && fail "something was uploaded into a published release"
ok "a release already published is left alone, and the job fails"

# latest.json, which installed copies read to find an update and check its signature.
UPDATER="$ROOT/.github/release/updater.sh"
complete "$DIR/signed" 1.2.4
for sig in "$DIR"/signed/*.sig; do printf 'sig-of-%s' "$(basename "$sig" .sig)" > "$sig"; done
printf 'Faster blame.\n\nAnd more.\n' > "$DIR/notes.md"
"$UPDATER" 1.2.4 "$DIR/signed" "https://example.invalid/v1.2.4" "$DIR/notes.md" >/dev/null \
    || fail "latest.json was not written for a complete signed release"
python3 - "$DIR/signed/latest.json" <<'PY' || fail "latest.json is not what the updater reads"
import json, sys
doc = json.load(open(sys.argv[1]))
assert doc["version"] == "1.2.4", doc["version"]
assert doc["notes"].startswith("Faster blame."), doc["notes"]
assert doc["pub_date"].endswith("Z"), doc["pub_date"]
want = {
    "linux-x86_64-appimage": "Coral_1.2.4_linux-x86_64.AppImage",
    "linux-x86_64-deb": "Coral_1.2.4_linux-x86_64.deb",
    "linux-aarch64-appimage": "Coral_1.2.4_linux-aarch64.AppImage",
    "linux-aarch64-deb": "Coral_1.2.4_linux-aarch64.deb",
    "darwin-x86_64-app": "Coral_1.2.4_macos-universal.app.tar.gz",
    "darwin-aarch64-app": "Coral_1.2.4_macos-universal.app.tar.gz",
    "windows-x86_64-nsis": "Coral_1.2.4_windows-x86_64-setup.exe",
    "windows-aarch64-nsis": "Coral_1.2.4_windows-aarch64-setup.exe",
}
# A plain os-arch key is what the updater falls back to for a copy it cannot place.
assert not any(k.count("-") == 1 for k in doc["platforms"]), sorted(doc["platforms"])
assert set(doc["platforms"]) == set(want), sorted(doc["platforms"])
for platform, name in want.items():
    entry = doc["platforms"][platform]
    assert entry["url"] == "https://example.invalid/v1.2.4/" + name, entry["url"]
    assert entry["signature"] == "sig-of-" + name, (platform, entry["signature"])
PY
ok "latest.json names every kind of install, its download and its signature"

rm "$DIR/signed/latest.json" "$DIR/signed/Coral_1.2.4_windows-aarch64-setup.exe.sig"
"$UPDATER" 1.2.4 "$DIR/signed" "https://example.invalid/v1.2.4" "$DIR/notes.md" >/dev/null 2>&1 \
    && fail "latest.json was written with a signature missing"
[ ! -e "$DIR/signed/latest.json" ] || fail "a partial latest.json was left behind"
ok "no latest.json is written when a signature is missing"

# The manifest's platforms are the build matrix's: a platform added to one and not the other
# would either never be checked or never be built.
# Read from the matrix's include list only: a step id elsewhere in the file looks the same.
built=$(awk '/^ +include:$/ { inside = 1; next }
             inside && /^ +- id: / { print $3; next }
             inside && /^ +[a-z-]+:/ && !/^ +(os|args):/ { exit }' \
        "$ROOT/.github/workflows/release.yml" | sort)
listed=$("$MANIFEST" --platforms | sort)
[ "$built" = "$listed" ] || fail "the manifest lists $(echo $listed) but release.yml builds $(echo $built)"
ok "the manifest names exactly the platforms release.yml builds"

# Every update download the manifest asks for has to be something the bundler is told to make.
# macOS updates from the .app in a tarball, which Tauri makes only when `app` is a target of
# its own: with `dmg` alone it built the disk image and no tarball, and 1.2.4's first tag
# failed for want of it.
python3 - "$ROOT/crates/coral-app/tauri.conf.json" <<'PY' || fail "a bundle the updater needs is not a bundle target"
import json, sys
targets = json.load(open(sys.argv[1]))["bundle"]["targets"]
missing = [t for t in ("deb", "appimage", "app", "nsis") if t not in targets]
assert not missing, f"tauri.conf.json does not build {missing}"
PY
ok "every kind of install that updates itself is a bundle target"

printf '%d passed\n' "$passed"
