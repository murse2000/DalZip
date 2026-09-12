#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
output="artifacts/DalZipFinder.appex/Contents"
mkdir -p "$output/MacOS"
cp native/macos/Info.plist "$output/Info.plist"
xcrun swiftc native/macos/FinderSync.swift -module-name DalZipFinder -target arm64-apple-macos12.0 -O -application-extension -framework Cocoa -framework FinderSync -Xlinker -e -Xlinker _NSExtensionMain -o "$output/MacOS/DalZipFinder"
codesign --force --sign "${DALZIP_SIGN_IDENTITY:--}" --entitlements native/macos/Entitlements.plist "artifacts/DalZipFinder.appex"
