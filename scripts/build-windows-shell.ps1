$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
cmake -S native/windows -B native/windows/build -A x64
if ($LASTEXITCODE) { throw "탐색기 확장 구성 실패" }
cmake --build native/windows/build --config Release
if ($LASTEXITCODE) { throw "탐색기 확장 빌드 실패" }
New-Item -ItemType Directory -Force src-tauri/resources | Out-Null
Copy-Item native/windows/build/Release/DalZipShell.dll src-tauri/resources/DalZipShell.dll -Force
