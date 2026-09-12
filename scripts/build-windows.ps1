$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
& (Join-Path $PSScriptRoot "build-windows-shell.ps1")
npm run tauri -- build --bundles nsis --config src-tauri/tauri.windows.conf.json
if ($LASTEXITCODE) { throw "Windows 패키지 빌드 실패" }

if ($env:TAURI_SIGNING_PRIVATE_KEY) {
  node scripts/package-updates.mjs windows
  if ($LASTEXITCODE) { throw "업데이트 서명 실패" }
}
