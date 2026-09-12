$ErrorActionPreference = "Stop"
if (!$env:GITHUB_ACTIONS) { throw "이 설치 회귀 검증은 일회용 GitHub Actions Windows 환경에서만 실행합니다." }
$version = (Get-Content src-tauri/tauri.conf.json | ConvertFrom-Json).version
$directory = Join-Path $env:RUNNER_TEMP "DalZipUpgradeQA"
$baseline = Join-Path $env:RUNNER_TEMP "DalZip-0.1.0.exe"
Invoke-WebRequest "https://github.com/murse2000/DalZip/releases/download/v0.1.0/DalZip-0.1.0-x64-setup.exe" -OutFile $baseline
function Run-Installer($file, $arguments) {
    $process = Start-Process $file -ArgumentList $arguments -PassThru
    if (!$process.WaitForExit(180000)) { $process.Kill(); throw "설치가 제한 시간 내에 끝나지 않았습니다." }
    if ($process.ExitCode -ne 0) { throw "설치 실패: $($process.ExitCode)" }
}
Run-Installer $baseline "/S /D=$directory"
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ShellLock {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    public static extern IntPtr LoadLibrary(string path);
    [DllImport("kernel32.dll")]
    public static extern bool FreeLibrary(IntPtr module);
}
'@
# 탐색기가 이전 확장 DLL을 사용 중인 상황을 실제 메모리 매핑으로 재현합니다.
$module = [ShellLock]::LoadLibrary((Join-Path $directory "DalZipShell.dll"))
if ($module -eq [IntPtr]::Zero) { throw "이전 확장 DLL을 로드하지 못했습니다." }
try {
    $installer = (Resolve-Path "artifacts/DalZip-$version-x64-setup.exe").Path
    Run-Installer $installer "/S /UPDATE /D=$directory"
    $expected = Join-Path $directory "shell/DalZipShell-$version.dll"
    if (!(Test-Path $expected)) { throw "새 버전의 확장 DLL이 설치되지 않았습니다." }
    $registered = (Get-Item "HKCU:\Software\Classes\CLSID\{627ED496-2F22-459E-A0EC-D2459E46C191}\InprocServer32").GetValue("")
    if ($registered -ne $expected) { throw "새 확장 DLL 등록 경로가 일치하지 않습니다." }
    Write-Output "이전 DLL이 사용 중인 상태에서 업데이트 설치 및 새 DLL 등록 성공"
} finally {
    [void][ShellLock]::FreeLibrary($module)
    if (Test-Path (Join-Path $directory 'uninstall.exe')) { Run-Installer (Join-Path $directory 'uninstall.exe') '/S' }
}
