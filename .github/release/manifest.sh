#!/usr/bin/env bash
#
# What a release must carry, and whether a directory carries it.
#
#   manifest.sh <version> <dir>   exits 1 naming every file <dir> is missing
#   manifest.sh --list <version>  the file names, one per line
#   manifest.sh --platforms       the platform ids, which are release.yml's matrix ids
#
# 1.2.2 reached GitHub with the Linux x86-64 files and nothing else. The bundle job already
# fails a platform that produces nothing; this is the check that all of them arrived.
# tests/release/run.sh runs it against made-up release directories.
set -euo pipefail

PLATFORMS=(linux-x86_64 linux-aarch64 macos-universal windows-x86_64 windows-aarch64)

# The installer, and the command-line front end where it is a download of its own. The .deb
# installs the CLI as well, but the tarball beside it is what an AppImage user reaches for.
files_for() {
    local v=$1 id=$2
    case "$id" in
        linux-*)   printf '%s\n' "Coral_${v}_${id}.deb" "Coral_${v}_${id}.AppImage" "coral_${v}_${id}.tar.gz" ;;
        macos-*)   printf '%s\n' "Coral_${v}_${id}.dmg" "coral_${v}_${id}.tar.gz" ;;
        windows-*) printf '%s\n' "Coral_${v}_${id}-setup.exe" "coral_${v}_${id}.exe" ;;
        *) echo "no files known for platform $id" >&2; exit 2 ;;
    esac
}

list() {
    local id
    for id in "${PLATFORMS[@]}"; do files_for "$1" "$id"; done
}

case "${1:-}" in
    --platforms) printf '%s\n' "${PLATFORMS[@]}"; exit 0 ;;
    --list) list "${2:?a version}"; exit 0 ;;
esac

version=${1:?a version}
dir=${2:?a directory}
missing=0
while read -r name; do
    if [ ! -e "$dir/$name" ]; then
        echo "::error::the release is missing $name"
        missing=$((missing + 1))
    fi
done < <(list "$version")

if [ "$missing" -gt 0 ]; then
    echo "$missing of $(list "$version" | wc -l) files are missing; not publishing" >&2
    exit 1
fi
echo "all $(list "$version" | wc -l) files are present"
