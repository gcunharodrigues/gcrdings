#!/usr/bin/env bash
#
# Builds gcrdings and replaces the installed copy, leaving exactly one.
#
# Without this the build folder keeps a second .app that Spotlight indexes, so
# Launchpad shows two identical icons and there is no way to tell which one is
# current. Worse, tauri-plugin-single-instance means opening the new copy while
# the old one runs just focuses the old one — the update looks like it did
# nothing.
#
# Recordings and the database in ~/Library/Application Support/gcrdings are
# never touched. Only caches and the duplicate bundle are removed.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bundle="$repo_root/target/debug/bundle/macos/gcrdings.app"
installed=/Applications/gcrdings.app

export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}

echo "==> Building"
pnpm --dir "$repo_root/frontend" tauri build --debug

echo "==> Quitting any running copy"
# A running instance would swallow the launch of the new one.
osascript -e 'tell application id "com.gcrdings.app" to quit' 2>/dev/null || true
sleep 2
pkill -f "gcrdings.app/Contents/MacOS/gcrdings" 2>/dev/null || true
sleep 1

echo "==> Replacing $installed"
rm -rf "$installed"
cp -R "$bundle" /Applications/

# The signature is what lets the app trust its own bundled ffmpeg. A copy that
# broke it would fail at import time, far from here.
codesign --verify --deep --strict "$installed"

echo "==> Removing the duplicate bundle and stale caches"
rm -rf "$bundle"
rm -rf ~/Library/Caches/com.gcrdings.app \
       ~/Library/WebKit/com.gcrdings.app \
       ~/Library/Saved\ Application\ State/com.gcrdings.app.savedState \
       ~/Library/HTTPStorages/com.gcrdings.app

echo "==> Refreshing Launchpad"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$installed"
killall Dock 2>/dev/null || true

echo
echo "Installed: $(stat -f '%Sm' -t '%d/%m %H:%M' "$installed/Contents/MacOS/gcrdings")"
echo "Copies on disk: $(mdfind "kMDItemFSName == 'gcrdings.app'" 2>/dev/null | wc -l | tr -d ' ')"
echo "Your recordings and database were not touched."
