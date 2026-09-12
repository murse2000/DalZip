# UnRAR 교차 빌드 수정

unrar_sys 0.5.8 (crates.io 배포본)을 포함합니다. build.rs의 cfg!(windows)는 빌드 호스트를 검사하여 macOS→Windows 빌드에서도 pthread를 연결하고 isnt.cpp를 제외하는 문제가 있습니다. CARGO_CFG_TARGET_OS / CARGO_CFG_TARGET_ENV로 실제 대상을 검사하도록 수정했습니다. MSVC 대상에는 정적 C++ 런타임을 사용하여 별도 MSVCP140.dll 설치 의존성을 없앴습니다. 압축 해제 알고리즘 코드는 변경하지 않았습니다.

원본: https://github.com/muja/unrar.rs / unrar_sys 0.5.8
