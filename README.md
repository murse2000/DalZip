# DalZip · by dalbear

Apple Silicon과 Windows x64용 로컬 압축 앱입니다. MultiXterm의 달베어·초승달 브랜드를 바탕으로 파란색 유리 재질 UI와 지퍼 폴더 아이콘을 적용했습니다. Tauri 2 / TypeScript / Rust를 사용합니다. 화면의 유리 효과는 플랫폼 공통 CSS로 구현하며, Apple SwiftUI의 네이티브 Liquid Glass API를 사용한 앱은 아닙니다.

## 실행과 설치

- macOS: `artifacts/DalZip.app` 또는 `artifacts/DalZip-0.1.1-arm64.dmg`. DMG를 열고 DalZip을 응용 프로그램 폴더로 옮깁니다.
- Windows: `artifacts/DalZip-0.1.1-x64-setup.exe`. Windows 11 x64용입니다. WebView2가 없는 경우 설치 프로그램이 해당 런타임을 설치합니다. Mac 교차 빌드의 UnRAR SIMD 코드는 AES-NI / SSE4.1을 지원하는 CPU를 대상으로 합니다.
- 파일 연결: 앱의 **설정 및 파일 연결**에서 확장자를 선택합니다. macOS는 Launch Services를 통해 설정하고, Windows는 연결 후보 등록 후 기본 앱 설정을 엽니다. Windows의 최종 기본 앱 선택은 사용자에게 있습니다.
- Finder 메뉴: Finder 확장이 포함되어 있습니다. 설정의 **Finder 확장 등록 및 설정 열기**를 사용하고, 시스템 설정 → 일반 → 로그인 항목 및 확장 프로그램 → Finder에서 DalZip을 켭니다. 운영체제 버전에 따라 ‘확장 프로그램’ 위치가 다를 수 있습니다.
- Windows 메뉴: 설치 프로그램이 탐색기 확장을 등록합니다. Windows 11에서는 **추가 옵션 표시**에서 확인합니다. 앱 설정에서도 다시 등록할 수 있습니다.

우클릭 메뉴는 `대표 항목명.zip으로 압축하기`로 표시됩니다. 다중 선택은 첫 항목을 대표 이름으로 사용합니다. 메뉴를 누르면 선택한 전체 항목을 압축 화면에 전달하고, 압축 버튼을 누를 때 대표 이름을 기본 저장 이름으로 제안합니다. 기존 이름이 있으면 덮어쓰지 않으며 다른 이름을 선택해야 합니다.

압축 파일만 선택하면 **별도 폴더에 압축 풀기** 메뉴도 표시됩니다. 각 압축 파일 옆에 별도 결과 폴더를 만들고 순서대로 해제합니다. 같은 이름의 기존 폴더는 덮어쓰지 않습니다. 암호가 필요하면 앱에서 입력하며, 해제 후 폴더 열기 설정을 따릅니다.

현재 결과물은 macOS 로컬 임시 서명 및 Windows 미서명 빌드입니다. 공개 배포용 Developer ID 서명·공증과 Windows 코드 서명은 별도 배포 인증서가 필요합니다. 이 환경에서 유효한 코드 서명 인증서는 발견되지 않았습니다. macOS에서 Finder 확장 로드, 동적 압축 메뉴의 파일 전달, 우클릭 압축 해제와 결과 폴더 자동 열기를 확인했습니다. Windows 설치·탐색기 동작은 Windows 실기기 검증이 남아 있습니다.

## 자동 업데이트

공개 저장소: https://github.com/murse2000/DalZip · 최신 설치 파일: https://github.com/murse2000/DalZip/releases/latest

0.1.1부터 실행 시와 6시간마다 HTTPS 업데이트 피드를 확인합니다. 새 버전이 있으면 알림을 표시하며, 설정의 **업데이트 확인**으로도 확인할 수 있습니다. **업데이트 설치** 버튼에 동의한 경우에만 파일을 다운로드하고 서명을 검증한 뒤 설치·재시작합니다. **나중에**는 다운로드하지 않습니다. 압축·해제 작업 중에는 업데이트를 설치하지 않습니다. 자동 확인의 네트워크 오류는 파일 작업을 방해하지 않으며 수동 확인에서는 오류를 안내합니다.

업데이트는 Windows x64 설치 파일과 macOS Apple Silicon 앱 번들을 구분합니다. macOS 번들에는 Finder 확장도 포함합니다. 서명이 잘못되면 설치하지 않습니다. 업데이트 서명은 OS 코드 서명·공증과 별개의 검증입니다. 0.1.0 사용자는 0.1.1을 한 번 직접 설치해야 합니다.

## 지원 형식

| 형식 | 목록 / 해제 | 생성 |
| --- | --- | --- |
| ZIP / ZIP64 | 지원, ZipCrypto·AES 암호화 해제 | 지원, 저장/빠름/표준/최대 |
| ZIPX | 지원하는 ZIP 코덱 범위 내에서 지원 | — |
| 7Z | 지원, 암호화 헤더 및 AES 해제 | 지원 |
| RAR | RAR4/5 해제, 암호화 해제 | — |
| TAR / TAR.GZ / TGZ | 지원 | TAR.GZ |
| TAR.BZ2 / TBZ2 / TAR.XZ / TXZ / TAR.ZST | 지원 | — |
| GZ / BZ2 / XZ / ZST 단일 스트림 | 지원, 실제 크기는 해제 중 계산 | — |

ZIP 엔진의 코덱: Stored, Deflate, Deflate64, BZip2, Zstd, LZMA, XZ. ZIPX의 JPEG/MP3 등 모든 전용 코덱을 지원하는 것은 아닙니다. 분할 ZIP·분할 RAR, RAR 생성, 암호화 압축 생성, 손상 복구, 압축 파일 내부 항목 수정은 제공하지 않습니다. TAR의 심볼릭 링크·하드 링크·특수 파일은 해제하지 않습니다. 이 버전은 권한·ACL·확장 속성·타임스탬프 전체 보존을 보장하지 않습니다.

## 작업 옵션

- **압축 해제 후 결과 폴더 열기**: 기본 켜짐. 설정에서 변경하면 다음 작업부터 적용합니다.
- **압축 완료·내용 검증 후 원본을 휴지통으로 이동**: 기본 꺼짐. 설정과 압축 화면에서 변경할 수 있습니다. 영구 삭제하지 않습니다. macOS에서는 파일 관리 API를 사용하며, 일부 OS 버전에서 휴지통의 ‘되돌려 놓기’ 메뉴가 없으면 파일을 직접 꺼내 복원할 수 있습니다.
- 원본 정리를 켜면 생성된 압축 파일을 임시 위치에 다시 해제하고, 파일 경로·크기 및 BLAKE3 내용 해시를 현재 원본과 비교합니다. 일치할 때만 휴지통으로 이동합니다. 이 과정은 추가 시간과 임시 디스크 공간을 사용합니다. 검증 실패 시 생성된 압축 파일과 원본을 보존합니다. 휴지통 이동 중 일부 항목에서 오류가 나면 결과 압축 파일을 유지하고 남아 있는 원본을 확인하도록 안내합니다.
- 비밀번호는 파일이나 설정에 저장하지 않습니다. 암호화된 목록을 연 경우 현재 열린 파일의 해제에 필요한 동안만 앱 메모리에 유지됩니다.

## 성능과 안전

ZIP 중앙 디렉터리와 열린 파일 핸들을 공유하고, 스레드마다 독립적인 파일 위치·읽기 버퍼를 사용합니다. 최대 4개 스레드가 병렬로 해제합니다. 원본 데이터를 통째로 메모리에 올리지 않습니다. 디렉터리 생성도 중복을 제거합니다.

- ZIP CRC / 7Z·RAR·스트림 체크섬 오류를 전파합니다.
- 절대 경로, `..`, Windows 예약 이름, 중복·대소문자·유니코드 정규화 충돌, 링크·특수 파일을 검사합니다.
- 해제는 기존 파일이 없는 독립된 새 폴더에서 실행합니다. 실패와 취소 시 그 작업의 임시 결과만 제거합니다. 전원 차단이나 강제 프로세스 종료까지 트랜잭션을 보장하는 것은 아닙니다.
- 기본 해제 한도 20 GiB, 설정 범위 1~2048 GiB, 파일당이 아닌 압축 파일 전체 한도입니다. 최대 항목 수는 100,000개입니다. 이는 출력 크기 제한이며 모든 외부 코덱의 메모리 할당 상한을 의미하지 않습니다.
- 출력 파일은 임시 파일에서 완성한 뒤 디스크 동기화하고, 기존 파일을 대체하지 않는 방식으로 게시합니다.
- 목록·압축·해제 모두 로컬에서 처리합니다. 앱은 파일을 서버로 전송하지 않습니다.

성능 결과와 재현 방법: [검증 기록](VALIDATION.md), `artifacts/benchmark.json`.

## 개발과 빌드

필요 환경: Node.js 24 이상, Rust stable 1.93 이상. macOS는 Xcode/Command Line Tools, Windows는 Visual Studio C++ Build Tools 및 CMake가 필요합니다.

```sh
npm ci
npm run app:dev
npm test
npm run check
```

macOS 패키지:

```sh
bash scripts/build-macos.sh
```

공개 서명 시 `DALZIP_SIGN_IDENTITY`를 지정합니다. 공증은 별도 배포 절차에서 수행해야 합니다. Finder 확장 코드는 `native/macos/FinderSync.swift`, 빌드는 `scripts/build-finder-extension.sh`입니다.

Windows에서 최초 `npm run check`/`npm test` 전에 `./scripts/build-windows-shell.ps1`로 탐색기 DLL을 생성합니다.

Windows 네이티브 패키지:

```powershell
./scripts/build-windows.ps1
```

macOS에서 Windows 교차 패키지:

```sh
brew install llvm lld nsis
cargo install cargo-xwin --locked
rustup target add x86_64-pc-windows-msvc
python3 scripts/build-windows-cross.py
```

Windows 탐색기 확장은 `native/windows/ShellExtension.cpp`의 `IExplorerCommand` 구현입니다. 설치/제거 등록은 `src-tauri/windows-hooks.nsh`에서 처리합니다. UnRAR의 교차 빌드 수정은 `src-tauri/vendor/README.md`에 기록했습니다.

`.github/workflows/build.yml`에는 macOS / Windows의 검증과 패키지 생성 작업이 들어 있습니다. 공개 저장소에서 실행할 수 있습니다. 버전 태그를 푸시하면 두 OS 검증과 패키지 생성이 성공한 뒤 설치 파일·서명·latest.json을 하나의 릴리스로 공개합니다.

## 아이콘과 라이선스

아이콘 원본: `assets/dalzip-icon.png`. ImageGen 내장 도구로 기존 MultiXterm 달베어 아이콘을 편집했습니다. 최종 프롬프트와 적용 경로는 `assets/README.md`에 기록했습니다.

서드파티 라이브러리 목록은 `licenses/DEPENDENCIES.md`, UnRAR 라이선스는 `licenses/UnRAR.txt`입니다. RAR 테스트 자료는 unrar-rs 0.5.8의 공개 테스트 자료이며 `tests/fixtures/UNRAR-RS-LICENSE`를 포함합니다.

## 다음 버전 배포

1. package.json, Cargo.toml, tauri.conf.json과 Finder 확장 Info.plist의 버전을 맞추고 Cargo.lock/package-lock.json을 갱신합니다.
2. `releases/v버전.md`에 변경 사항을 작성하고 테스트를 실행합니다.
3. 변경 사항을 커밋하고 해당 `v버전` 태그를 푸시합니다. GitHub Actions가 검증·서명·릴리스 공개를 수행합니다.

업데이트 개인 키는 저장소에 넣지 않습니다. GitHub Secrets의 `TAURI_SIGNING_PRIVATE_KEY`를 사용하며, 이 Mac의 로컬 키는 `~/.config/dalzip/updater.key`에 보관합니다. 키를 잃으면 기존 설치본에 같은 신뢰 키로 업데이트를 배포할 수 없으므로 별도로 안전하게 백업해야 합니다. 로컬 서명 빌드는 `TAURI_SIGNING_PRIVATE_KEY`를 키 파일 경로로 지정해 기존 빌드 스크립트를 실행합니다. 두 OS 파일을 artifacts에 모은 뒤 `node scripts/release-manifest.mjs`로 피드를 생성합니다.
