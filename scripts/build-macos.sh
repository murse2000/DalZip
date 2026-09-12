#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(node -p "JSON.parse(require('fs').readFileSync('src-tauri/tauri.conf.json')).version")
npm run tauri -- build --bundles app
bash scripts/build-finder-extension.sh
mkdir -p src-tauri/target/release/bundle/macos/DalZip.app/Contents/PlugIns
cp -R artifacts/DalZipFinder.appex src-tauri/target/release/bundle/macos/DalZip.app/Contents/PlugIns/
codesign --force --sign "${DALZIP_SIGN_IDENTITY:--}" src-tauri/target/release/bundle/macos/DalZip.app
mkdir -p artifacts
# 공개 배포 서명은 DALZIP_SIGN_IDENTITY로 지정하며, 기본 빌드는 로컬 서명입니다.
ditto src-tauri/target/release/bundle/macos/DalZip.app artifacts/DalZip.app
staging=$(mktemp -d "${TMPDIR:-/tmp}/dalzip-dmg.XXXXXX")
trap 'rm -rf "$staging"' EXIT
ditto artifacts/DalZip.app "$staging/DalZip.app"
ln -s /Applications "$staging/Applications"
hdiutil create -volname DalZip -srcfolder "$staging" -ov -format UDZO artifacts/DalZip-${version}-arm64.dmg
codesign --verify --deep --strict artifacts/DalZip.app
shasum -a 256 artifacts/DalZip-${version}-arm64.dmg > artifacts/SHA256SUMS-macos.txt

if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  node scripts/package-updates.mjs macos
fi
