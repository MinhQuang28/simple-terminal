#!/bin/bash
# Package a Repodeck release: build the .app, zip it, and compute its sha256. Publishing to
# GitHub is opt-in (--publish) so this never makes an outward-facing change by accident. Usage:
#   script/package-release.sh            # build + zip + sha256 (local only)
#   script/package-release.sh --publish  # also create the GitHub release and upload the zip
set -euo pipefail

cd "$(dirname "$0")/.."

PUBLISH=0
[ "${1:-}" = "--publish" ] && PUBLISH=1

APP_NAME="Repodeck"
# Single source of truth for the version: the package version in Cargo.toml.
VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
[ -n "$VERSION" ] || { echo "error: could not read version from Cargo.toml" >&2; exit 1; }
TAG="v${VERSION}"

APP="target/release/bundle/osx/${APP_NAME}.app"
ZIP="dist/${APP_NAME}.zip"

echo "==> building ${APP_NAME} ${VERSION}"
script/build-app.sh

echo "==> zipping ${APP} -> ${ZIP}"
mkdir -p dist
rm -f "$ZIP"
# ditto --keepParent preserves the .app bundle structure and code signature (plain `zip` can mangle it).
ditto -c -k --keepParent "$APP" "$ZIP"

SHA="$(shasum -a 256 "$ZIP" | awk '{print $1}')"
echo "==> sha256: ${SHA}"

if [ "$PUBLISH" -eq 1 ]; then
    command -v gh >/dev/null || { echo "error: gh CLI not found" >&2; exit 1; }
    echo "==> creating GitHub release ${TAG} and uploading ${ZIP}"
    # --generate-notes auto-builds release notes from merged commits; clobber re-uploads on re-run.
    gh release create "$TAG" "$ZIP" --title "$TAG" --generate-notes 2>/dev/null \
        || gh release upload "$TAG" "$ZIP" --clobber
    echo "==> published: $(gh release view "$TAG" --json url -q .url)"
else
    echo ""
    echo "==> done (local). Publish with: script/package-release.sh --publish"
fi
