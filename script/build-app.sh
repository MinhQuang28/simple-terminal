#!/bin/bash
# Build Repodeck.app, sign it, and optionally install it into /Applications.
#
#   script/build-app.sh            build + sign into target/release/bundle/osx/Repodeck.app
#   script/build-app.sh --install  also replace /Applications/Repodeck.app
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="Repodeck"
APP="target/release/bundle/osx/${APP_NAME}.app"
ENTITLEMENTS="macos/repodeck.entitlements"
INSTALL_DIR="/Applications"
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"

install=false
case "${1:-}" in
    --install) install=true ;;
    "") ;;
    *) echo "usage: $0 [--install]" >&2; exit 2 ;;
esac

# Only the .app: a .dmg made here would wrap the bundle before it is signed.
echo "==> cargo bundle --release --format osx"
cargo bundle --release --format osx

# CFBundleVersion must increase between builds of the same version, or LaunchServices keeps serving the
# copy it already registered (stale Services menu entries, old icon). A timestamp always increases.
BUILD_NUMBER="$(date +%Y%m%d%H%M)"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion ${VERSION}.${BUILD_NUMBER}" "$APP/Contents/Info.plist"

# Signing, best identity first. All routes use the hardened runtime so the notarizable shape is the
# one that gets tested locally.
#  1. Developer ID (set DEVELOPER_ID_APP): notarization-ready, with a secure timestamp.
#  2. Stable local identity (script/setup-signing-cert.sh): permissions survive rebuilds.
#  3. Ad-hoc, so the bundle at least runs.
SIGN_HASH="$(security find-identity "$HOME/Library/Keychains/login.keychain-db" 2>/dev/null \
    | awk '/Repodeck Local Signing/ {print $2; exit}' || true)"
if [ -n "${DEVELOPER_ID_APP:-}" ]; then
    echo "==> signing with Developer ID (${DEVELOPER_ID_APP})"
    codesign --force --sign "$DEVELOPER_ID_APP" --options runtime --timestamp \
        --entitlements "$ENTITLEMENTS" "$APP"
    echo "    notarize with: ditto -c -k --keepParent \"$APP\" Repodeck.zip && xcrun notarytool submit --wait Repodeck.zip"
elif [ -n "$SIGN_HASH" ]; then
    echo "==> signing with stable local identity ($SIGN_HASH)"
    codesign --force --sign "$SIGN_HASH" --options runtime --timestamp=none \
        --entitlements "$ENTITLEMENTS" "$APP"
else
    echo "==> ad-hoc signing (run script/setup-signing-cert.sh for a stable signature)"
    codesign --force --sign - --options runtime --timestamp=none \
        --entitlements "$ENTITLEMENTS" "$APP"
fi
codesign --verify --strict --verbose=1 "$APP"

if ! $install; then
    echo "==> built: $(pwd)/$APP"
    echo "    install with: $0 --install"
    exit 0
fi

if pgrep -xq "$(basename "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist")")"; then
    echo "==> note: Repodeck is running; quit it (⌘Q) and reopen to use the new build"
fi
echo "==> installing into ${INSTALL_DIR}"
# Stage the copy next to the old bundle and swap with renames, so the Dock and LaunchServices
# never see a half-copied bundle (one without its icon yet) and cache a placeholder icon for it.
STAGING="${INSTALL_DIR}/.${APP_NAME}.app.staging"
OLD="${INSTALL_DIR}/.${APP_NAME}.app.old"
rm -rf "$STAGING" "$OLD"
ditto "$APP" "$STAGING"
if [ -e "${INSTALL_DIR}/${APP_NAME}.app" ]; then
    mv "${INSTALL_DIR}/${APP_NAME}.app" "$OLD"
fi
mv "$STAGING" "${INSTALL_DIR}/${APP_NAME}.app"
rm -rf "$OLD"

# One LaunchServices record per bundle id: a second one (the build output) makes the Services menu
# and `open -b` pick either copy.
"$LSREGISTER" -u "$APP" >/dev/null 2>&1 || true
"$LSREGISTER" -f "${INSTALL_DIR}/${APP_NAME}.app"
/System/Library/CoreServices/pbs -update
echo "==> installed: ${INSTALL_DIR}/${APP_NAME}.app"
