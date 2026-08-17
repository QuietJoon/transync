---
type: How-To Guide
title: transync serve로 번역된 데모 번들 제공하는 방법
description: `transync serve`를 사용하여 루프백 HTTP를 통해 --html-out 또는 --out-dir 데모 번들을 제공하고, 두 패널이 동기화되는지 확인하며, 깔끔하게 중지합니다.
tags: [cli, serve, sync, SCN-13]
audience: operator
language: ko
sources:
  - { id: en-source, resource: manual/how-to/operator/en/serve-the-demo-bundle.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:02Z
synced_hash: a23a3e11fe59700337dc6a8ec09a6dc72353e7a38085ec9967417870727045ff
---
# `transync serve`로 번역된 데모 번들 제공하는 방법

`transync`가 제공하는 서버를 사용하여 이미 생성된 데모 번들(`transync translate --html-out <dir>` 또는 `--out-dir <dir>`로 생성)을 브라우저에서 열고 스크롤 동기화되도록 합니다.

## 시작하기 전에

- `transync translate --html-out <dir>` 또는 `--out-dir <dir>`로 생성한 번들이 있어야 합니다. 번들이 없는 경우 [CLI로 문서 번역하는 방법](../../user/ko/translate-a-document-with-the-cli.md)을 참조하세요.
- 서버가 반드시 필요한 이유는 번들의 셸(`index.html`)이 무엇인가를 마운트하기 전에 `fetch()`로 `source.html`, `target.html` 및 `alignment.json`을 가져오기 때문입니다. `file://` URL로 열 경우 대부분의 브라우저에서 해당 요청이 차단되거나 불투명 오리진(opaque-origin)으로 처리되어 패널이 나타나지 않습니다.

## 번들 디렉터리 위치 확인

- `--html-out <dir>` 실행: 6개의 번들 파일(`index.html`, `source.html`, `target.html`, `alignment.json`, `sync.js`, `purify.min.js` — `crates/transync-cli/src/output.rs`에서 `HTML_BUNDLE_ENTRIES`라고 부르는 세트)이 `<dir>`에 직접 위치합니다. `<dir>`을 제공하세요.
- `--out-dir <dir>` 실행: 동일한 6개 파일이 한 단계 아래인 `<dir>/html/`에 위치하며, 최상위 형제 파일인 `out.md`, `alignment.json`, `validation-report.json`은 `<dir>` 자체에 함께 위치합니다. 브라우저는 `html/` 내부의 사본이 필요합니다 — **`--rendered`가 `<dir>`이 아닌 `<dir>/html`을 가리키도록 하세요**. 상위 디렉터리를 제공하는 것이 가장 흔한 실수입니다: `/`가 `index.html`이 없는 디렉터리로 해석되므로 첫 번째 요청이 404가 됩니다.

## 서버 실행하기

```
transync serve --rendered <bundle-dir>        # or <out-dir>/html
```

디렉터리는 인자이므로 `cd`가 필요하지 않으며, `--rendered` 자체가 심볼릭 링크일 수도 있습니다 — 시작 시 루트가 한 번 정규화(canonicalized)되며 모든 요청은 링크의 대상을 기준으로 측정됩니다.

stderr에 두 줄이 출력됩니다:

```
transync serve: listening on http://127.0.0.1:7470/ — serving /abs/path/to/bundle
transync serve: press Ctrl-C to stop.
```

`127.0.0.1`과 `7470`은 플래그 기본값(`--bind`, `--port`)입니다. 유용한 변형:

- `--port 0`은 OS에 사용 가능한 포트를 요청합니다. `listening on` 줄은 커널이 실제로 바인딩한 주소를 보고하므로, 그 줄이 선택된 포트가 나타나는 유일한 장소입니다.
- `--bind 0.0.0.0`(또는 임의의 비루프백 주소)도 작동하며, 먼저 경고를 출력합니다: `transync serve: WARNING: 0.0.0.0 is not a loopback address — everything under <root> is reachable from other machines on this network.`
- `serve`가 출력하는 모든 내용은 **stderr**로 전송되며 stdout은 비어 있습니다. 바인딩된 포트가 필요한 스크립트는 stderr를 읽습니다. `serve`에는 `--quiet` 또는 `--verbose` 플래그가 없습니다 — `crates/transync-cli/src/main.rs`에서 기본 상세도 하한으로 고정합니다.

전체 플래그, 상태, 헤더 및 MIME 테이블은 [CLI 참조 문서](../../../reference/user/en/cli.md)에 위치하며, 이 페이지에서는 이를 반복하지 않습니다.

## 열어서 패널 동기화 확인하기

`http://127.0.0.1:7470/`(또는 `listening on` 줄에 출력된 주소)를 엽니다. `/` 및 `/`로 끝나는 모든 대상은 해당 디렉터리의 `index.html`로 해석되므로 단순 루트 주소만으로 작동합니다. 한 패널을 스크롤하면 블록 ID에 따라 다른 패널도 함께 이동해야 합니다; 팔로워는 스크롤을 멈춘 후 대략 250 ms 이내에 간격의 약 95%를 메웁니다.

가장 간단한 방법부터 시작하는 세 가지 점검 사항:

- **마운트 판정.** 성공 시 `mountSync`는 *두* 패널 요소 모두에 `__transyncController`를 태깅합니다; 거부 시 `null`을 반환하고 아무것도 태깅하지 않습니다. CLI 번들에서 패널은 `#source` 및 `#target`이므로 `document.getElementById('source').__transyncController`의 존재 여부가 "마운트되었는가"에 대한 결정적 답변입니다 — 이것이 헤드리스 스위트가 읽는 값입니다.
- **앵커 (Anchors).** `document.querySelectorAll("[data-sync-id]")`가 두 패널에서 동일한 비어 있지 않은 ID 세트를 반환합니다.
- **콘솔.** 이 엔진 버전과 일치하는 맵의 경우, `web/js/sync.js`는 `console.debug`를 통해 `transync: alignment map loaded (schema_version=<the map's own version>)`를 출력합니다 — 일부 브라우저는 "Verbose" 레벨 토글 뒤에 `debug` 줄을 숨깁니다. `1.2.0`은 현재 와이어 버전(`crates/transync-syntax/src/align.rs`의 `ALIGNMENT_SCHEMA_VERSION`)입니다; [정렬 맵 스키마 참조 문서](../../../reference/developer/ko/alignment-map-schema.md)를 참조하세요. 성공/실패로만 보지 말고 이 줄을 주의 깊게 읽으세요: 1.2.0보다 *신규*인 동일 메이저 버전 맵은 디버그 줄을 `console.warn`(`… is newer than this engine (1.2.0); proceeding, but sync may be incomplete`)으로 교체하면서 계속 동기화하므로, 디버그 줄의 부재나 경고의 존재 자체가 실패를 의미하지는 않습니다.

창 크기를 조정할 때 후속 스크롤이 필요하지 않습니다. 엔진이 스스로 재정렬됩니다: 두 패널의 `ResizeObserver`, `document.fonts.ready`, 그리고 어느 한쪽 패널 내부 `<img>`의 캡처 단계 `load`/`error`가 각각 하나의 결합된 재계산을 예약하여 두 앵커 세트를 다시 수집하고 팔로워를 다시 구동합니다. 패널의 HTML을 *교체*하는 경우에만 여전히 `controller.destroy()`와 새로운 `mountSync`가 필요합니다.

## 서버 중지하기

Ctrl-C 또는 슈퍼바이저나 셸 `kill`의 `SIGTERM`을 사용합니다. `transync serve: shutting down.`이 출력되고 수신기가 삭제되어 새로운 요청이 도달하지 않으며, 진행 중인 연결에는 2초의 유예 기간이 부여되고, 그 이후에도 여전히 열려 있는 모든 연결은 `… connection(s) still open after 2s; closing anyway.` 줄과 함께 중단됩니다. 프로세스는 **0**으로 종료되므로 `transync serve` 다음에 실행되는 스크립트는 의도적인 중지를 실패가 아닌 성공으로 읽습니다.

## 서버가 제공하는 항목과 제공하지 않는 항목

`--rendered`를 번들 이외의 다른 항목으로 지정하기 전에 알아두면 좋은 사항:

- **`GET` 및 `HEAD` 전용.** 다른 모든 메서드는 `Allow: GET, HEAD`와 함께 405 응답을 반환합니다. 업로드, 실행, 프록시는 지원되지 않습니다.
- **디렉터리 목록 표시 불가.** 끝에 슬래시가 *없는* 디렉터리 이름은 일반 파일이 아닌 것으로 해석되어 404가 됩니다; 끝에 슬래시가 있으면 해당 디렉터리의 `index.html`로 해석됩니다.
- **두 개의 독립적인 격리 레이어.** 요청 대상은 어떤 퍼센트 디코딩보다 *먼저* `/`를 기준으로 분할되고 각 세그먼트는 정확히 한 번 디코딩됩니다; 디코딩된 `.` 또는 `..`는 403이며 디코딩된 구분자 또는 NUL은 400입니다. 그런 다음 후보는 정규화되어 정규화된 루트에 대해 구성 요소별로 검사됩니다. 이 레이어는 번들 외부를 가리키는 심볼릭 링크를 403으로 변환합니다. 거부는 **거절할 뿐 절대 잘라내어 맞추지(clamp) 않습니다** — 로그의 403은 정규화된 요청이 아닌 실제 시도입니다.
- **심볼릭 링크는 파일 레이블을 변경할 수 없음.** `Content-Type`은 정규화된 경로에서 계산되므로 `page.html -> data.json`은 JSON으로 제공됩니다.
- **고정된 확장자 테이블, 스니핑 불가.** 테이블에 지정되지 않은 모든 항목에 대해 `application/octet-stream`이 사용되며, 모든 응답에 `X-Content-Type-Options: nosniff`가 추가됩니다.
- **모든 응답에 `Cache-Control: no-store` 적용.** 재생성된 번들이 캐시된 `sync.js`보다 뒤처질 수 없습니다; `transync translate`를 다시 실행한 후 강제 새로고침할 필요가 없습니다.
- **Keep-alive 없음.** 설계상 연결당 정확히 하나의 요청만 허용됩니다.
- **데모 규모의 제한.** 진행 중인 연결 최대 128개, 15초 요청 헤드 타임아웃, 8 KiB 요청 헤드 상한. HTTP Range 미지원(미디어 탐색 불가) 및 TLS 미지원 — 두 가지 모두 서버 출하 시 범위 외로 지정되었습니다.
- **하나의 문서화된 잔여 레이스 조건.** 다른 로컬 프로세스가 `canonicalize`와 `open` 사이에 항목을 교체할 수 있습니다. 해당 창은 시스템 콜 하나의 범위이며 제공되는 디렉터리 내부의 로컬 쓰기 권한이 필요합니다; 이를 완전히 닫는 것(`openat`/`O_NOFOLLOW` 순회)은 루프백 데모 서버의 범위를 벗어난 것으로 판단되어 다시 발견되도록 남겨두지 않고 `crates/transync-cli/src/serve_cmd/conn.rs`에 기록되었습니다.

## 문제 해결

| 증상 | 의미 | 조치 방법 |
|---|---|---|
| 종료 코드 `2`, `transync serve: … is not a directory; --rendered takes the bundle directory.` (또는 `cannot serve <path>: …`) | `--rendered`가 존재하지 않거나, 읽을 수 없거나, 파일을 지정함 | 번들 *디렉터리*를 가리키도록 하세요 — `--html-out`의 경우 `<dir>`, `--out-dir`의 경우 `<dir>/html` |
| 종료 코드 `5`, `transync serve: cannot bind <addr>: …` | 주소/포트를 바인딩할 수 없음 — 보통 이미 사용 중이거나 이 호스트가 소유하지 않은 주소임 | 다른 `--port`를 선택하거나, OS가 할당하는 포트를 위해 `--port 0`을 사용하세요 |
| clap 사용법 오류와 함께 종료 코드 `1` | `--rendered`가 누락되었거나 `--bind`를 IP 주소로 파싱할 수 없음 | `--rendered`를 제공하고, `--bind`에 호스트 이름이 아닌 리터럴 주소를 지정하세요 |
| `/` 요청 시 `404` | 제공되는 디렉터리에 `index.html`이 없음 — 보통 `<out-dir>/html` 대신 `--rendered <out-dir>`을 사용한 경우 | `--rendered`를 한 단계 아래를 가리키도록 수정하세요 |
| 존재하는 경로에 대해 `404` | 대상이 끝 슬래시 없이 디렉터리를 지정했거나 일반 파일이 아닌 것을 지정함 | 끝 슬래시를 추가하거나 파일 이름을 지정하세요 |
| `403` | `.`/`..` 세그먼트이거나 심볼릭 링크를 통해 루트 외부로 해석된 경로임 | 예상된 거부입니다; 실제 파일을 제공되는 루트 내부로 이동하세요 |
| `405` | `GET`/`HEAD` 이외의 메서드로 요청이 전송됨 | 예상된 거부입니다; 서버에서 수정할 사항이 없습니다 |
| 왼쪽 패널에 `transync: DOMPurify missing — refusing to mount unsanitized HTML` 표시 | `purify.min.js`가 로드되지 않음 | 거의 항상 잘못된 디렉터리 문제임 — 위의 `--out-dir`/`html/` 구분을 참조하세요 |
| 페이지가 멈춘 후 왼쪽 패널에 `transync: timed out loading <path> after 20000 ms` 표시 | 서버가 요청을 수락하고 교착 상태에 빠짐 (앞단의 멈춘 프록시 또는 중단된 프로세스) | `fetchOk`는 모든 아티팩트를 20초로 제한합니다; 서버를 재시작하고 해당 포트에 다른 프로세스가 바인딩되어 있지 않은지 확인하세요 |
| 콘솔 경고 `the … pane is not the offsetParent of its blocks` | 패널이 비정적(non-static) `position`을 잃음 — 스타일이 변경된 CSS | CSS를 수정하세요. 이 경고는 동기화가 동작하지 않는 것이 아니라 **잘못 정렬(misaligned)**됨을 의미하는 유일한 경고입니다 |
| 콘솔에서 맵 행의 ID가 일치하지 않는다고 경고하고 아무것도 동기화되지 않음 | 앵커는 *동일한* `data-sync-id`로 쌍을 이룹니다. 대역 내(in-band) 맵(동일 메이저, 1.2.0보다 크지 않음)에서 `target_block_id`가 `source_block_id`와 다른 행은 손상된 것이며 전체 맵이 거부됩니다 | 맵을 재생성하거나 제3자 생성기를 수정하여 소스 ID를 두 필드 모두에 쓰도록 하세요. *전방 마이너(forward-minor)* 맵만 관대한 처리(경고 출력 후 소스 ID로 쌍 만들기)를 받습니다 |

반복되는 경고 계열 — 중복된 `data-sync-id`, DOM 앵커가 없는 맵 행, 알 수 없는 `sync_role`, 비동일성 `target_block_id` — 은 각각 최대 5회 발생까지 출력되고 억제된 집계가 표시되므로, 손상된 대형 번들이 콘솔을 도배하지 않도록 합니다.

## 다른 정적 서버를 사용하고자 하는 경우

번들에는 서버 측 로직이 없으므로(`docs/architecture/persistence-and-files.md` 참조) `npx serve`, `nginx`, CI 아티팩트 서버 등 모든 정적 파일 서버를 사용할 수 있습니다. 단, `transync serve`가 제공하는 두 가지 기능인 `Cache-Control: no-store`와 [wasm 데모](./build-the-wasm-demo.md)에 필요한 `application/wasm` 콘텐츠 타입을 포기해야 합니다.

## 관련 문서

- [wasm 렌더링+편집 데모 빌드 및 실행 방법](./build-the-wasm-demo.md) — 동일한 서버가 호스팅할 수 있는 별도의 브라우저 내 렌더링 경로(`web/demo-wasm.html`).
- [transync 시작하기](../../../tutorials/operator/ko/getting-started.md) — 이 레시피가 마지막 단계인 엔드투엔드 첫 실행 가이드.
- [번역 실행 진단하는 방법](../../user/ko/diagnose-a-translation-run.md) — 서버 제공보다는 번들 자체가 잘못되어 보일 때.
- `web/SMOKE.md` — 이 레시피가 준비 단계인 수동 스모크 체크리스트. 이것의 자동화된 후속 도구인 `scripts/test-browser.sh`는 이 동일한 서버를 구동합니다: 빌드된 바이너리를 `TRANSYNC_SERVE_BIN`으로 전달하고, `web/playwright.config.js`는 `serve --rendered <fixture> --bind 127.0.0.1 --port 4319`를 실행하며, 바이너리가 제공되지 않은 경우에만 소형 Node 대체품으로 폴백합니다.
- `docs/architecture/contracts.md` §6 — 권위 있는 serve 계약.
