---
type: How-To Guide
title: WASM 렌더+편집 데모 빌드 및 실행 방법
description: scripts/build-wasm.sh로 transync-wasm 모듈을 빌드하고, 번역된 문서 옆에 데모 디렉터리를 구성하고, `transync serve`로 호스팅하며, 데모의 부팅 진단 결과를 확인합니다.
tags: [wasm, demo, ADR-0019, DCR-0020]
audience: operator
language: ko
sources:
  - { id: en-source, resource: manual/how-to/operator/en/build-the-wasm-demo.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:02Z
synced_hash: 9df267db68fa52eba9078d02b310e2fb52437119be898b6d32b5cf5593d83e77
---
# WASM 렌더+편집 데모 빌드 및 실행 방법

`web/demo-wasm.html`은 WebAssembly로 컴파일된 CLI와 동일한 Rust 렌더러 (Renderer)(ADR-0019)를 사용하여 **브라우저에서** 두 패널을 렌더링하고, 번역된 블록 (Block)을 편집하며 실시간으로 다시 렌더링되는 모습을 볼 수 있게 해줍니다. 이는 CLI의 `--html-out` 번들과는 별개의 데모입니다(CLI 번들은 의도적으로 WASM을 포함하지 않음). 따라서 자체 빌드 단계와 파일 레이아웃이 필요합니다.

## 사전 조건

- `PATH`에 `wasm-pack`이 있어야 함.
- `PATH`에 **>= 121** 버전의 `wasm-opt`([binaryen](https://github.com/WebAssembly/binaryen) 제공)가 있어야 함. wasm-pack 자체에 캐시된 binaryen은 **충분하지 않습니다** — 해당 카피는 117 버전을 제공하며, 이는 최신 rustc의 bulk-memory 출력을 거부합니다. 스크립트는 해당 캐시 카피로 실행이 이탈하지 않도록 절대 경로로 `wasm-opt`를 한 번 확인합니다.
- 아직 추가하지 않았다면 `rustup target add wasm32-unknown-unknown`.
- 이전 `transync translate` 실행에서 생성된 번역 문서 — `out.md` 및 `alignment.json`([CLI로 문서 번역하기](../../user/ko/translate-a-document-with-the-cli.md) 참조) — 및 이들을 생성한 원문 (Source) 파일.

두 도구 전제 조건이 충족되지 않을 경우 무시하고 건너뛰는 대신 설치 힌트와 함께 오류를 발생시킵니다. 한 가지 실패 모드는 버전 문제처럼 보이지만 실제로는 그렇지 않으므로 미리 숙지할 가치가 있습니다. 검사 로직은 `version <n>` 토큰을 기준으로 binaryen 배너를 파싱합니다(연도로 시작하는 벤더 배너를 버전으로 잘못 읽던 현상 대응). `--version` 출력에 해당 토큰이 포함되지 않은 **래핑되거나 벤더링된 `wasm-opt`**는 충분히 최신 버전이더라도 다음과 같은 메시지와 함께 거부됩니다:

```
[build-wasm] could not parse a 'version <n>' from '<path> --version' — need binaryen >= 121. It printed:
```

그 뒤에 해당 도구의 원본 출력이 그대로 표시되어 실제 출력 내용을 확인할 수 있습니다. 해결책은 정식 binaryen `wasm-opt`를 `PATH`에서 가장 앞에 두는 것입니다. 이 검사를 건너뛰는 재정의 옵션은 없습니다.

## 1. 모듈 빌드

```bash
./scripts/build-wasm.sh
```

**모든 게이트를 통과할 때까지는 아무것도 설치되지 않습니다.** wasm-pack 빌드, `wasm-opt -Oz` 패스, 크기 측정 및 두 예산 검사는 모두 실행별 스테이징 디렉터리에서 수행되며, 그 이후에만 `web/wasm/`이 교체됩니다.

순서대로 실행되는 작업:

1. `wasm-pack build crates/transync-wasm --target web --profile wasm-release --no-opt --no-pack --out-dir "$STAGING"`, 여기서 `$STAGING`은 스테이징 루트의 `mktemp -d` 하위 디렉터리입니다. 해당 루트는 `TRANSYNC_WASM_STAGING`이며, 기본값은 `/Volumes/Temp/claude/transync-wasm-pkg`이고 기본값의 부모 디렉터리가 존재하지 않을 때는 `${TMPDIR:-/tmp}/transync-wasm-pkg`로 폴백됩니다. 재정의값은 공유 삭제 가드(`scripts/lib/workdir-guard.sh`)에 의해 검증됩니다. 실행별 하위 디렉터리가 있어 두 개 이상의 동시 빌드가 공존할 수 있습니다. `--no-opt`는 선택 사항이 아닌 필수 사항입니다: wasm-pack은 `wasm-release`와 같은 사용자 정의 프로필의 최적화 키를 무시합니다.
2. 최신 rustc 출력에 필요한 bulk-memory / reference-types / nontrapping-float-to-int / sign-ext / mutable-globals 제안 플래그와 함께 직접 호출되는 `wasm-opt -Oz`.
3. 두 상한 기준에 대한 크기 측정 — **1,840,000 B (압축 전)** 및 **760,000 B (gzip)** (2026-08-05 소유자 승인, `transync-wasm`의 `rlib` 크레이트 (Crate) 타입이 제거되었을 때 하향 조정됨). 모든 실행은 `[build-wasm] size: raw=<n>B gzip=<n>B (budget: raw<=1840000B gzip<=760000B)`를 출력합니다. 스크립트의 자체 주석에는 현재 측정값이 1,639,520 B (압축 전) / 671,795 B (gzip)로 기록되어 있으며 상한선은 이보다 약 12 % (압축 전) 및 13 % (gzip) 높게 설정되어 있습니다. 상한 초과는 올려 잡을 숫자가 아니라 디자인 (Design) 신호로 서술됩니다.
4. `mkdir` 락(`web/.wasm.publish.lock`) 하의 두 가지 이름 변경 작업을 통한 게시: 검증된 쌍이 `web/wasm/`의 *형제* 디렉터리로 복사되어 거기서 다시 검사되고, 기존 `web/wasm/`은 `web/.wasm.backup.*`으로 이동되며, 해당 형제 디렉터리가 `web/wasm/`이 됩니다. 형제 디렉터리는 정적 서버가 모듈을 읽어오는 디렉터리가 되므로 umask가 일반 디렉터리에 부여했을 권한 모드로 의도적으로 chmod됩니다.

신뢰할 수 있는 세 가지 결과:

- **빌드가 실패하더라도 이전의 정상 모듈이 유지됩니다.** 모든 실패 경로 — wasm-pack이 글루(glue)나 모듈을 출력하지 않음, raw 또는 gzip 예산 초과, 빈 스테이징 파일, 확인 불가 또는 너무 오래된 `wasm-opt`, 파싱 불가능한 binaryen 배너, 너무 오래 유지된 게시 락 — 는 교체 *전에* 0이 아닌 값을 반환하며 종료되므로 `web/wasm/`에는 통과했던 마지막 모듈이 그대로 유지됩니다. 두 크기 실패 메시지에도 다음과 같이 명시됩니다: `… $OUT_DIR left untouched.`
- **글루와 모듈은 항상 하나의 버전 결합 쌍으로 설치됩니다.** 브라우저 탭이나 병렬 Playwright 실행 중 어느 것도 혼합된 쌍을 관찰할 수 없으며, 두 이름 변경 사이의 간격을 조회하는 리더는 아무것도 찾지 못합니다.
- **이후 `web/wasm/`은 이번 빌드가 배출한 세트만 정확히 유지합니다** — 과거 빌드가 이번 빌드에서 더 이상 생성하지 않는 이름으로 작성했던 아티팩트는 남아있지 않습니다.

성공 시: `[build-wasm] OK — module + glue landed in <repo>/web/wasm`, `transync_wasm.js`(글루) 및 `transync_wasm_bg.wasm`(모듈)을 포함합니다. 둘 다 교체용 형제 디렉터리와 마찬가지로 `web/wasm/` 및 `web/.wasm.*`을 통해 gitignore 처리됩니다.

## 2. 데모 디렉터리 구성

페이지는 나란히 위치한 세 파일(`source.md`, `out.md`, `alignment.json`)을 상대 경로로 가져오고, `vendor/purify.min.js` 및 `./js/wasm-demo.js`를 로드하며, 해당 모듈은 글루를 `../wasm/transync_wasm.js`로, 동기화 (Sync) 엔진을 `./sync.js`로 임포트합니다. 디렉터리를 다음과 같이 구성합니다:

```
demo/
├── demo-wasm.html          # web/demo-wasm.html
├── source.md                # the ORIGINAL document you translated
├── out.md                   # the translated Markdown (--output, or --out-dir's out.md)
├── alignment.json           # the alignment map (--map, or --out-dir's alignment.json)
├── js/
│   ├── wasm-demo.js         # web/js/wasm-demo.js
│   └── sync.js              # web/js/sync.js
├── vendor/
│   └── purify.min.js        # web/vendor/purify.min.js
└── wasm/
    ├── transync_wasm.js      # web/wasm/transync_wasm.js  (from step 1)
    └── transync_wasm_bg.wasm # web/wasm/transync_wasm_bg.wasm (from step 1)
```

```bash
mkdir -p demo/js demo/vendor demo/wasm
cp web/demo-wasm.html demo/
cp web/js/wasm-demo.js web/js/sync.js demo/js/
cp web/vendor/purify.min.js demo/vendor/
cp web/wasm/transync_wasm.js web/wasm/transync_wasm_bg.wasm demo/wasm/
cp <your-source-document>.md demo/source.md
cp <your-translated-output>.md demo/out.md
cp <your-alignment-map>.json demo/alignment.json
```

글루와 모듈은 하나의 디렉터리 내에 형제로 유지되어야 합니다 — wasm-bindgen 글루는 자체 URL을 기준으로 `.wasm` 파일을 확인합니다. 이는 `scripts/test-browser.sh`가 픽스처 디렉터리에 미러링하는 것과 동일한 구조입니다.

## 3. 호스팅 및 열기

```bash
transync serve --rendered demo
```

`http://127.0.0.1:7470/demo-wasm.html`을 엽니다. 범용 정적 서버 대신 `transync serve`를 사용하십시오: 해당 콘텐츠 타입 테이블은 `.wasm`에 대해 `application/wasm`을, `.js`/`.mjs`에 대해 `text/javascript`를 지정하여 구체적으로 이 구동을 호스팅할 수 있도록 합니다. `WebAssembly.instantiateStreaming`은 다른 타입으로 제공되는 모듈의 컴파일을 거부합니다. 서버의 플래그, 거부 조건 및 제한 사항은 [번역 데모 번들 호스팅 방법](./serve-the-demo-bundle.md)을 참조하십시오.

## 4. 부팅 진단 결과 확인

데모는 `document.body.dataset.demoState`에 자체 상태를 기록합니다: 진입 시 `booting`, 두 패널이 모두 마운트되고 클릭 핸들러가 활성화되면 `ready`, 게이트에서 거부되면 `fatal`. 해당 속성은 헤드리스 스위트가 사용하는 준비 완료 신호이자 "모듈 로딩 중"과 "거부됨"을 구별하는 가장 빠른 방법입니다. 치명적(fatal) 메시지는 원문 (Source) 패널의 내용을 대체하고, 대상 (Target) 패널을 비우며, 오류 스트립을 채우고 `console.error`로 기록됩니다.

게이트는 다음 순서대로 실행되며 각각 실패 시 닫힘(fail-closed) 방식으로 작동합니다:

1. 페이지의 `#source`, `#target`, `#editor` 엘리먼트가 존재함 — 렌더링할 위치가 없는 1메가바이트가 넘는 모듈을 가져오기 *전*에 가장 먼저 확인됩니다.
2. WASM 모듈이 초기화됨.
3. 모듈의 스키마 버전이 데모의 스키마 버전과 일치함.
4. `window.DOMPurify`가 존재함.
5. 세 아티팩트 모두 제한 시간 내에 페치됨.
6. `alignment.json`이 JSON으로 파싱됨.
7. 맵의 `schema_version`이 자체 검증을 통과함.
8. `blocks`가 존재하는 경우 배열 형태임.
9. 중복 행이 없음.
10. 편집 가능한 모든 행의 `target_range`를 슬라이스할 수 있음.
11. `render_pair`가 성공함.
12. DOMPurify가 정화 중 예외를 발생시키지 않음.
13. 동기화 (Sync) 엔진이 맵을 승인함.

마지막 두 항목은 렌더링이 이미 존재하는 경우 다르게 동작합니다: 부팅 시 예외를 발생시키는 DOMPurify나 거부된 맵은 치명적이지만, 세션 중간(편집 재빌드 중)에는 둘 다 오류 스트립에 보고되고 마지막 정상 렌더링이 화면에 유지됩니다.

이 중 두 가지는 흔히 잘못 해석되며, 잘못 해석하면 잘못된 파일로 안내될 수 있습니다:

- **`schema mismatch: wasm <x> vs demo <y>`**는 *모듈의* `schema_version()`을 데모 JS에 컴파일된 자체 상수(`web/js/wasm-demo.js` 내 `KNOWN_SCHEMA = "1.2.0"`)와 비교합니다. **가져온 정렬 (Alignment) 맵은 이 검사에 전혀 관여하지 않으므로**, 맵을 재생성해도 해결되지 않습니다. 해결책은 현재 트리에 맞춰 모듈을 다시 빌드(1단계)하거나 `web/js/wasm-demo.js`를 업데이트하는 것 중 뒤처진 쪽을 처리하는 것입니다.
- **가져온 맵**은 다른 진단 결과와 함께 별도의 게이트에서 판단됩니다: 메이저 버전이 1이 아닌 `schema_version`은 치명적입니다(`… is not major 1 (demo speaks 1.2.0) — refusing to mount`). 반면 동일 메이저 버전의 *더 새로운* 버전은 `console.warn` 경고만 발생시키고(`proceeding, but rendering may be incomplete`) 데모가 계속 진행됩니다.

언급할 가치가 있는 두 가지 추가 실패 사례:

- `render failed: alignment map has an unusable target byte range: …` — Rust 렌더러 (Renderer)는 데모의 자체 편집 가능 행 게이트가 확인하지 않는 `html` 및 건너뛴 행을 포함하여, 슬라이스할 수 없는 바이트 범위(`RenderError::UnusableRange`)를 지닌 **모든** 행에 대해 거부합니다. 뒤집히거나 범위를 벗어나거나 UTF-8 중간에 걸친 범위는 클램핑되지 않고 거부됩니다. 정렬 (Alignment) 맵을 재생성하십시오.
- `timed out loading <path> after 20000 ms` — `fetchOk`가 각 아티팩트의 제한 시간을 설정하며, 하나의 실패가 공유 `AbortController`를 통해 나머지 두 형제 아티팩트를 중단시킵니다. 요청을 수락한 후 멈춰버리는 서버는 데모를 더 이상 `booting` 상태에 묶어둘 수 없습니다.

## 5. 편집 시도

대상 (Target) (번역된) 패널에서 편집 가능한 블록 (Block)을 클릭합니다. 해당 마크다운이 하단의 편집기 패널에 로드됩니다. 이를 편집하면 짧은 디바운스 후 브라우저에서 전체 루프가 다시 실행되고(재생성 → 재정렬 → 재렌더링) 패널이 업데이트됩니다.

편집은 번역된 쪽의 블록 (Block) *페이로드*로만 범위가 제한됩니다. 블록 세트 자체는 데모가 절대 편집하지 않는 원문 (Source) 문서를 파싱하여 얻으므로, 어떤 편집으로도 동기화 (Sync) 앵커를 생성하거나 제거할 수 없습니다.

**거부된** 편집은 모델에 반영되지 않습니다. 제출 내용은 페이로드 맵의 스테이징된 클론이며, `state.payloads`는 재빌드 *및* 그 이후의 마운트를 통과했을 때만 갱신됩니다. 실패한 재빌드는 비치명적 오류 스트립에 보고되고(`transync: rebuild failed: …`) 화면에는 마지막 정상 렌더링이 유지되어 세션이 계속됩니다. 입력한 텍스트는 블록 ID를 키로 하여 `state.drafts`에 보관되며, 해당 블록을 다시 클릭할 때 편집기로 다시 불러오므로 다른 곳을 클릭했다가 돌아와도 손실되지 않습니다.

한 가지 주의 사례: 재빌드가 성공했지만 재생성된 문서가 더 이상 원문 (Source)의 최상위 블록 수와 일치하지 않는 경우, 패널은 여전히 마운트되고 동일한 오류 스트립에 `structure_warning`이 표시됩니다. 보고 있는 렌더링은 저하된 렌더링입니다 — 형상이 변경된 블록은 원본 마크다운을 보여주며, 그 이후의 블록은 올바른 앵커 아래에서 밀린 내용을 보여줄 수 있습니다. 변경 사항을 취소하는 것이 완전한 복구 방법이며 다음 번 깨끗한 재빌드 시 스트립이 지워집니다.

## 빌드가 실패할 때

- 크기 초과 시 **`$OUT_DIR left untouched`** 발생 — 어떤 부분이 늘어났는지 조사하십시오. 조사하는 동안에도 `web/wasm/`에는 마지막 정상 모듈이 남아있어 데모가 계속 작동합니다.
- **게시 락.** `web/.wasm.publish.lock`은 두 번의 이름 변경 동안만 유지되는 락 *디렉터리*이며 몇 밀리초 동안만 유지됩니다. 빌드는 최대 30초 동안 기다린 후 실행 중인 빌드가 없을 때 해당 디렉터리를 수동으로 제거하라는 안내와 함께 실패합니다. 이것이 스크립트의 유일한 수동 복구 단계이며, 빌드가 교체 중간에 강제 종료되었음을 의미합니다.
- **남겨진 형제 파일.** `kill -9`로 인해 `web/.wasm.staging.*` 또는 `web/.wasm.backup.*`이 남을 수 있습니다. 이들은 `web/.wasm.*`에 의해 gitignore 처리되며 실행 중인 빌드가 없을 때 안전하게 삭제할 수 있습니다. EXIT 트랩은 통상적으로 데모에 모듈이 전혀 없는 상태로 두지 않고 `web/.wasm.backup.*`에서 이동되었던 모듈을 복원합니다.

## 자동으로 검증하기

`scripts/test-browser.sh`는 자체 실행의 일부로 `scripts/build-wasm.sh`를 실행하며, 스크래치 픽스처 디렉터리 내에 생성된 CLI 번들 위에 이 데모 단계를 구성하고 헤드리스로 구동합니다. 이 스위트는 3개 스펙 파일(`web/tests/scn13.spec.js` (12개), `web/tests/engine.spec.js` (9개), `web/tests/wasm.spec.js` (9개))에 걸쳐 총 30개의 테스트로 구성됩니다. Playwright는 스크립트가 `TRANSYNC_SERVE_BIN`을 통해 바이너리를 전달할 때 `transync serve`를 사용하여 루프백 `127.0.0.1:4319`에서 픽스처를 호스팅하고, 그렇지 않은 경우 소형 Node 대역을 사용합니다.

## 관련 문서

- [CLI로 문서 번역하기](../../user/ko/translate-a-document-with-the-cli.md) — 이 데모에 필요한 `out.md` / `alignment.json` 쌍을 생성합니다.
- [번역 데모 번들 호스팅 방법](./serve-the-demo-bundle.md) — WASM 빌드가 전혀 필요 없는 CLI 자체 `--html-out` 번들 및 3단계에서 사용되는 서버 설명입니다.
- [정렬 맵 스키마 레퍼런스](../../../reference/developer/ko/alignment-map-schema.md) — 데모의 맵 게이트가 검사하는 내용 설명입니다.
- `docs/decisions/0019-wasm-demo-layer.md` — 이것이 왜 CLI 번들의 일부가 아닌 별도의 웹 전용 데모인지 설명합니다.
