#!/usr/bin/env bash
#
# Publishes <dir> as the draft release for <tag>.
#
#   publish.sh <tag> <version> <dir> <notes-file>
#
# A draft for the tag already there, from an earlier run of the same tag, gets the files
# replaced. A release already *published* for the tag is left alone and the job fails: 1.2.2's
# files were uploaded into a release made by hand, which went out with one platform in it and
# a name the workflow never gives. Whoever published it decides what happens to it.
set -euo pipefail

tag=${1:?a tag}
version=${2:?a version}
dir=${3:?a directory}
notes=${4:?a notes file}

if draft=$(gh release view "$tag" --json isDraft --jq .isDraft 2>/dev/null); then
    if [ "$draft" != "true" ]; then
        echo "::error::$tag is already published; not uploading into it"
        exit 1
    fi
    gh release upload "$tag" "$dir"/* --clobber
else
    gh release create "$tag" "$dir"/* --draft --title "Coral $version" --notes-file "$notes"
fi
