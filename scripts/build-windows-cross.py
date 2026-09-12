"""macOS에서 LLVM, cargo-xwin, NSIS를 사용해 Windows x64 패키지를 빌드합니다."""
import os
from pathlib import Path
import re
import shlex
import subprocess

root = Path(__file__).resolve().parents[1]
os.chdir(root)
env = os.environ.copy()
env['PATH'] = '/opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/lld/bin:' + env['PATH']
settings = subprocess.check_output(['cargo', 'xwin', 'env', '--target', 'x86_64-pc-windows-msvc', '--manifest-path', 'src-tauri/Cargo.toml'], env=env, text=True)
for line in settings.splitlines():
    match = re.fullmatch(r'export ([A-Za-z0-9_]+)="(.*)";', line)
    if match:
        env[match[1]] = match[2]
# Windows 11 x64의 SIMD 명령어를 사용하는 UnRAR 코드를 clang-cl로 컴파일합니다.
env['CXXFLAGS'] = '/clang:-maes /clang:-mssse3 /clang:-msse4.1'
env['CXXFLAGS_x86_64_pc_windows_msvc'] += ' ' + env['CXXFLAGS']
output = root / 'src-tauri/resources/DalZipShell.dll'
output.parent.mkdir(exist_ok=True)
(root / 'artifacts').mkdir(exist_ok=True)
subprocess.run(['clang-cl', *shlex.split(env['CXXFLAGS_x86_64_pc_windows_msvc']), '/std:c++17', '/utf-8', '/O2', '/MT', '/LD', 'native/windows/ShellExtension.cpp', '/Foartifacts/DalZipShell.obj', '/link', '/DEF:native/windows/ShellExtension.def', '/IMPLIB:artifacts/DalZipShell.lib', '/OUT:' + str(output), 'ole32.lib', 'shell32.lib', 'shlwapi.lib', 'advapi32.lib', 'uuid.lib'], env=env, check=True)
# Tauri가 Windows용 설치 파일을 만들도록 대상과 빌드 러너를 함께 지정합니다.
subprocess.run(['npm', 'run', 'tauri', '--', 'build', '--runner', 'cargo-xwin', '--target', 'x86_64-pc-windows-msvc', '--bundles', 'nsis', '--config', 'src-tauri/tauri.windows.conf.json'], env=env, check=True)

if env.get("TAURI_SIGNING_PRIVATE_KEY"):
    import json
    version = json.loads((root / "src-tauri/tauri.conf.json").read_text())["version"]
    subprocess.run(["node", "scripts/package-updates.mjs", "windows", f"src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/DalZip_{version}_x64-setup.exe"], env=env, check=True)
