# DalZip 아이콘

- 생성 방식: 내장 ImageGen 도구, 참조 이미지 편집
- 참조: `MultiXterm/Assets/DalBearTerm.png`
- 최종 원본: `assets/dalzip-icon.png`
- UI 적용: `public/dalzip-icon.png`
- 패키지 적용: `src-tauri/icons`의 PNG, ICNS, ICO

초기 프롬프트: 기존 크림색 달베어 얼굴·검은 눈·금색 초승달·남색 3D 스타일을 보존하고, 터미널 표시를 파란 반투명 압축 폴더와 큰 은색 지퍼로 교체한다. 곰의 눈을 가리지 않으며 글자를 넣지 않는다.

최종 배경 보정 프롬프트:

> Edit this icon. Keep the bear, crescent moon, and blue ZIP zipper folder identity. REMOVE ALL CHECKERBOARD patterns. Do not attempt transparency. Output a fully opaque square image. Fill the ENTIRE 1024x1024 canvas, including all four corners and edges, with uniform deep midnight navy #0b1020. No checker pattern, no transparency simulation. Keep the rounded glass blue tile large within this solid dark navy canvas. No text. Final asset for dark desktop app.

재생성된 이미지는 실제 투명 PNG가 아닌 남색 배경의 불투명 PNG입니다. UI는 둥근 모서리로 표시합니다.
