#!/usr/bin/env bash
#
# Writes <dir>/latest.json, which an installed Coral reads to find an update.
#
#   updater.sh <version> <dir> <download-base-url> <notes-file>
#
# Each kind of install gets the download it installs from and the signature Tauri's updater
# checks that download against before installing anything. The keys name the installer as well
# as the platform, and there is deliberately no plain `linux-x86_64`: the updater falls back to
# that for a copy it cannot place, which would put an AppImage over a .deb or over a build from
# source. Both macOS architectures take the one universal app. Nothing is written if a signature is missing: an entry without one is an
# update every installed copy would refuse, after downloading it.
# tests/release/run.sh holds it to a made-up signed release.
set -euo pipefail

version=${1:?a version}
dir=${2:?a directory}
base=${3:?a download base url}
notes=${4:?a notes file}

python3 - "$version" "$dir" "$base" "$notes" <<'PY'
import datetime, json, pathlib, sys

version, folder, base, notes = sys.argv[1], pathlib.Path(sys.argv[2]), sys.argv[3], sys.argv[4]
platforms = {}
for arch in ("x86_64", "aarch64"):
    platforms[f"linux-{arch}-appimage"] = f"Coral_{version}_linux-{arch}.AppImage"
    platforms[f"linux-{arch}-deb"] = f"Coral_{version}_linux-{arch}.deb"
    platforms[f"darwin-{arch}-app"] = f"Coral_{version}_macos-universal.app.tar.gz"
    platforms[f"windows-{arch}-nsis"] = f"Coral_{version}_windows-{arch}-setup.exe"
entries = {}
for platform, name in platforms.items():
    sig = folder / (name + ".sig")
    if not sig.is_file():
        sys.exit(f"::error::no signature for {name}; not writing latest.json")
    entries[platform] = {"signature": sig.read_text().strip(), "url": f"{base}/{name}"}

# The first paragraph of the release notes: the updater shows it, and the whole changelog
# section is the release page's job.
first = pathlib.Path(notes).read_text().strip().split("\n\n")[0].strip()
doc = {
    "version": version,
    "notes": first,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": entries,
}
(folder / "latest.json").write_text(json.dumps(doc, indent=2) + "\n")
print(f"latest.json: {version} for {len(entries)} platforms")
PY
