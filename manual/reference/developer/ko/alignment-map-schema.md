---
type: Reference
title: 정렬 맵 (Alignment-map) JSON 스키마
description: 정렬 맵의 내구성 있는 유선 형태 — 최상위 키, 모든 블록 레벨 필드, 스키마 버전 정책, 및 규범적인 페어링 규칙.
tags: [alignment-map, wire-format, sync, reference, ADR-0001, DCR-0016]
audience: developer
language: ko
sources:
  - { id: en-source, resource: manual/reference/developer/en/alignment-map-schema.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:02Z
synced_hash: a89f04e686938a7c5657a4b9acceb0ec921d5d5c7339479bdba63273eaa8cb16
---
# 정렬 맵 (Alignment-map) JSON 스키마

정렬 맵은 파이프라인이 생성하기 위해 존재하는 산출물입니다: 어느 쪽도 마크다운을 재파싱하지 않고 렌더링된 두 패널이 함께 스크롤될 수 있도록 하는 블록별 대응 관계입니다. 이는 내구성 있는 유선 계약(wire contract)입니다 — 파괴적 변경에는 디자인 변경 기록(DCR) 및 `schema_version` 승상이 필요합니다.

Rust 타입은 `crates/transync-syntax/src/align.rs`의 `build_alignment_map`에 의해 생성되는 `transync::AlignmentMap`입니다. JSON 형태는 다음 표면들에 나타납니다:

| 표면 | 형태 |
|---|---|
| `transync::TranslationOutput.alignment_map` | 메모리 내 Rust 값 |
| `transync translate --map <path>` | 서식이 적용되어 출력된(pretty-printed) JSON 파일 |
| `transync translate --out-dir <dir>` | 해당 디렉터리의 `alignment.json` |
| `--html-out` 번들 및 `--out-dir`의 `html/` 하위 디렉터리 | `alignment.json`, 6개 번들 파일 중 하나; `index.html`이 자기 옆에서 가져옴 |
| `transync-wasm`의 `render_pair` / `rebuild` | wasm 경계를 넘는 JSON 문자열 |

두 CLI 경로 모두 `serde_json::to_vec_pretty`로 직렬화하므로 어느 쪽이든 파일 바이트는 동일합니다. 플래그 및 종료 코드는 [CLI 참조 문서](../../user/ko/cli.md)에 있습니다.

## 최상위 키

| 키 | JSON 타입 | 값 |
|---|---|---|
| `schema_version` | string | HEAD 기준 `\"1.2.0\"`. 아래를 참조하십시오. |
| `document_id` | string | 소문자 16진수 16자리 — 파싱된 원문 텍스트에 대한 SipHash-1-3 (키 `0, 0`). |
| `source_language` | string | 리터럴 `\"auto\"`를 포함하여 `TranslateOptions::source_language` 그대로의 문자열. 이 레이블을 파싱, 검증 또는 대소문자 변환하는 것은 없습니다. |
| `target_language` | string | 동일한 조건 하의 `TranslateOptions::target_language` 그대로의 문자열. |
| `detected_source_language` | string 또는 `null` | 이 실행에 대해 제공자 엔벨로프가 보고한 언어, 또는 프로바이더 배치를 **0개** 디스패치하고 캐시에서 모든 유닛을 제공받은 실행의 경우 문서 메타데이터 저장소에서 재플레이된 값. 둘 다 해당되지 않을 때 `null`. |
| `generator` | object | `{ \"name\": \"transync\", \"version\": \"<crate version>\" }`. 버전은 `transync-syntax` 자체의 패키지 버전이며, 이는 워크스페이스 버전(HEAD 기준 `0.3.0`)입니다. |
| `blocks` | array | 원문 순서대로 정렬된 최상위 원문 블록당 하나의 행. [블록 행](#블록-행)을 참조하십시오. |
| `validation_summary` | object | 실행 레벨 집계. [validation_summary](#validation_summary)를 참조하십시오. |

어떤 필드도 serde 기본값을 가지지 않으므로 `AlignmentMap`으로 누락되거나 잘못 입력된 필드가 있는 JSON을 역직렬화하는 Rust 소비자는 해당 필드의 이름과 함께 실패합니다. 알 수 없는 필드는 거부되지 않고 무시됩니다 — 이 덕분에 이전 마이너 버전에 고정된 빌드가 더 새로운 맵을 읽을 수 있습니다.

## `schema_version`

현재 유선 버전은 **`1.2.0`**입니다. 이 버전의 단일 Rust 신뢰 소스는 `crates/transync-syntax/src/align.rs`에 있는 `ALIGNMENT_SCHEMA_VERSION`이며, 공개 표면에는 `transync::ALIGNMENT_SCHEMA_VERSION`으로도 노출되어 wasm 모듈의 `schema_version()` 내보내기가 반환하는 값이기도 합니다.

버전 이력(모두 추가적임): `1.1.0`은 `\"skipped\"` `block_kind` 값을 추가했고(DCR-0013), `1.2.0`은 `\"html\"` 값을 추가했습니다(ADR-0018 / DCR-0016).

**정책.** 필드는 유전적 버전 (semver) 방식을 따릅니다. 메이저 승상은 파괴적 변경입니다. 마이너 또는 패치 승상은 필드 및 열거형 값을 추가할 수 있습니다. 소비자는 알 수 없는 메이저 버전을 거부해야 하며, 동일한 메이저 버전 내의 알 수 없는 마이너 또는 패치 버전은 삼키지 않고 표출하며 승인해야 합니다.

**버전 문자열 형태.** 두 JS 게이트 모두 각 구성 요소가 1–9자리 숫자인 정확한 `major.minor.patch` 형태를 요구합니다. `\"1\"`, `\"1.foo\"`, `\"1garbage\"` 및 400자리 숫자의 구성 요소는 모두 잘못 형성된 것으로 거부됩니다 — 숫자 제한은 지나치게 긴 구성 요소가 `Infinity`가 되어 영구적으로 포워드 표류로 읽히는 것을 방지하기 위해 존재합니다.

**각 소비자가 수행하는 작업:**

| 소비자 | 자체 상숫값 | 동일 버전 또는 동일 메이저의 이전 버전 | 동일 메이저의 더 새로운 버전 | 알 수 없는 메이저 또는 잘못 형성됨 |
|---|---|---|---|---|
| `web/js/sync.js` — `loadAlignment` | `KNOWN_SCHEMA = { major: 1, minor: 2, patch: 0 }` | `console.debug`: `transync: alignment map loaded (schema_version=<v>)`; 마운트됨 | `console.warn`: `…is newer than this engine (1.2.0); proceeding, but sync may be incomplete`; 마운트되며 두 가지 행 규칙이 완화됨 (아래 참조) | `console.warn`: `rejecting alignment map with unknown major schema_version=<v>`; `mountSync`가 `null`을 반환하며 패널은 연결되지 않은 상태로 남음 |
| `web/js/wasm-demo.js` — `alignmentSchemaVerdict` | `KNOWN_SCHEMA = \"1.2.0\"` | 마운트됨 | `console.warn`: `…is newer than this demo (1.2.0); proceeding, but rendering may be incomplete` | 치명적 패널: `alignment map schema_version=<v> is not major 1 (demo speaks 1.2.0) — refusing to mount` |

wasm 데모는 부팅 초기에 관련 없는 두 번째 검사를 실행합니다: 모듈의 `schema_version()`과 자체 `KNOWN_SCHEMA` 상수를 비교하고 `schema mismatch: wasm <x> vs demo <y>`와 함께 실패합니다. 가져온 맵은 해당 비교에 참여하지 않습니다.

**포워드 표류는 `sync.js`에서 정확히 두 가지 행 규칙을 완화하며**, 둘 다 경고를 수반합니다: 인식되지 않는 `sync_role`은 스크롤 앵커로 취급되며, 동일하지 않은 `target_block_id`는 원문 id에 의해 쌍을 이룹니다. 각 경고는 처음 5회 발생 시 출력된 후 억제된 집계 수치와 함께 한 번 더 출력됩니다. 대역 내 맵(동일한 메이저, `1.2.0`보다 최신이 아님)에서 두 값 중 하나라도 발견되면 손상으로 간주되어 맵이 거부됩니다.

버전의 3개 JS 미러는 `crates/transync-cli/tests/sync_js_drift.rs`에 의해 Rust 상수에 용접되어 있습니다 (`sync_js_known_schema_matches_the_rust_alignment_schema_version`은 `web/js/sync.js` 및 CLI 임베디드 사본을 다룸; `wasm_demo_js_known_schema_matches_the_rust_alignment_schema_version`은 데모를 다룸). 동일한 파일이 `sync.js`의 `KNOWN_SYNC_ROLES`를 Rust `SyncRole` 열거형에 용접하고, 워크스페이스 및 CLI 임베디드 `sync.js`가 바이트 단위로 동일함을 단언합니다.

## 블록 행

`blocks`는 번역 유닛이 되지 않는 블록 및 DOM 앵커를 전달하지 않는 블록을 포함하여 원문 순서대로 최상위 원문 블록당 하나의 행을 보유합니다. 중첩된 콘텐츠에 대한 행은 존재하지 않습니다: 파서는 잎-블록(leaf-block) 방식이므로 리스트 항목 자체가 최상위 블록이며 블록 아래의 어떤 것도 주소 지정되지 않습니다.

| 필드 | JSON 타입 | 값 |
|---|---|---|
| `source_block_id` | string | 블록의 안정적인 id. |
| `target_block_id` | string | 이 프로젝트가 내보내는 모든 행에서 `source_block_id`와 동일함 — 스키마 1.x의 규범적 표준. |
| `block_kind` | string | 블록 종류의 케밥-케이스(kebab-case) 유선 형태. |
| `source_order` | number (u32) | 원문 순서상 블록의 0-기반 인덱스. |
| `target_order` | number (u32) | 이 프로젝트가 내보내는 모든 행에서 `source_order`와 동일함. |
| `source_range` | object | 원문 문서에 대한 `{ \"start\": <byte>, \"end\": <byte> }`. |
| `target_range` | object | 재생성된 마크다운에 대한 `{ \"start\": <byte>, \"end\": <byte> }`. |
| `sync_role` | string | `anchor`, `container`, `child-only` 또는 `non-sync`. |
| `fallback_status` | string | `translated`, `preserved`, `partially_translated` 또는 `fallback_source`. |
| `parent_id` | string 또는 `null` | 항상 `null`. 예약됨. |

행은 누락 없이 오름차순 `source_order`로 나타나므로 배열 인덱스와 `source_order`는 파이프라인이 내보낸 모든 맵에 대해 일치합니다. 렌더러는 `source_order`를 **두** 패널 모두에서 `data-order` 속성으로 찍어 넣습니다; 렌더링 경로상의 어떤 것도 `target_order`를 읽지 않습니다.

### `source_block_id`, `target_block_id`, 및 페어링 규칙

Id는 `<kind-code>-<NNNN>` 형태를 갖습니다: 종류 접두사 — `h1`…`h6`, `p`, `t`, `c`, `li`, `q`, `hr`, `img`, `html`, `x` — 뒤에 문서 전반의 1-기반 카운터가 오며, 4자리로 0이 채워지고 9999를 초과하면 더 넓어집니다. 카운터는 모든 종류에 걸쳐 실행되므로 `h1-0001` 뒤에 `p-0002`가 옵니다. 두 패널 모두에서 블록의 렌더링된 래퍼에 설정되는 `data-sync-id` 속성도 동일한 문자열입니다.

**패널은 동일한 id에 의해 짝을 이루며, 이는 스키마 1.x의 규범입니다.** 블록 id는 번역을 거쳐도 살아남으므로(ADR-0001), 두 문서는 동일한 이름 하에 동일한 블록 세트를 보유합니다; 블록의 파트너는 다른 패널에서 *동일한* `data-sync-id`를 전달하는 요소입니다. `build_alignment_map`은 두 필드 모두에 하나의 `BlockId`를 작성하며, `transync-syntax::render`는 모든 속성과 두 패널의 `data-sync-id`를 `source_block_id` 기반으로 키 지정합니다 — 렌더링 경로상의 어떤 것도 `target_block_id`를 읽지 않습니다.

원문/대상 간접 참조는 `parent_id` 및 `child-only` `sync_role`과 동일한 의미에서 **예약**되어 있습니다: 두 문서의 블록 세트가 실제로 갈라질 수 있는 향후 개정을 위해 보유되며, 해당 개정은 스키마 승상으로 도착할 것입니다. 그때까지는 아무것도 표현하지 않으며, "동일성"을 나타내는 두 번째 방식이 되어서는 안 됩니다.

`web/js/sync.js`는 자체 절반을 강제합니다. `target_block_id`가 `source_block_id`와 다른 비어 있지 않은 문자열인 행은 엔진이 수행하지 않을 페어링을 설명하므로, 이러한 행을 지닌 대역 내 맵은 거부됩니다 — `mountSync`가 `null`을 반환하고 패널은 연결되지 않은 상태로 남으며 콘솔 메시지가 두 id를 지정합니다. 포워드 표류 하에서는 동일한 행이 경고 처리되고 원문 id에 의해 짝을 이룹니다. 누락되거나 `null`이거나 비어 있는 `target_block_id`는 위반이 아닙니다; 필드를 생략하는 것은 동일성에 모순되는 아무런 표현도 하지 않습니다.

### `block_kind`

`data-block-kind` DOM 속성과 동일한 블록 종류의 케밥-케이스 유선 형태: `heading-1`, `heading-2`, `heading-3`, `heading-4`, `heading-5`, `heading-6`, `paragraph`, `table`, `code-block`, `list-item`, `blockquote`, `thematic-break`, `image`, `html`, `skipped`.

`skipped` (스키마 1.1.0)는 파이프라인이 번역 가능한 것으로 모델링하지 않는 최상위 원문 노드입니다 — 프론트 매터, 각주 정의, 지원되지 않는 노드. 프로바이더에게 절대 전송되지 않고 원문 바이트가 축자적으로 접합되며 두 패널 모두에서 이스케이프된 비활성 `<pre data-skipped="<label>">` 자리 표시자로 렌더링됩니다. 기저 노드 종류를 지칭하는 레이블은 `block_kind`가 아닌 해당 DOM 속성에 존재합니다.

`html` (스키마 1.2.0)은 블록 레벨 원시-HTML 노드이며 번역 가능한 종류입니다: 텍스트 세그먼트가 추출되어 번역되고 다시 접합됩니다. 특정 html 블록이 유닛이 되었는지 여부는 블록별로 결정되며, 해당 행의 `fallback_status` 및 `validation_summary`에서 카운트되는지 여부에 나타납니다.

JS 동기화 엔진은 `[data-sync-id]`만 읽고 인식하지 못하는 `block_kind` 값을 허용하므로, 종류 추가는 마이너 승상에 해당합니다.

### `sync_role`

| 값 | 배출 대상 | 의미 |
|---|---|---|
| `anchor` | 구분선(thematic break) 및 인용구(block quote)를 제외한 모든 종류 — 제목, 단락, 테이블, 코드 블록, 리스트 항목, 이미지, html 블록, 및 `skipped` 자리 표시자 | 행이 두 패널 모두에서 스크롤 앵커 역할을 함 |
| `container` | 인용구 | 스크롤 앵커 역할을 하며, 래퍼가 다른 콘텐츠를 보유함 |
| `child-only` | 없음 | 향후 중첩 앵커 체계를 위해 **예약됨** (DCR-0007이 파서를 잎-블록으로 남겨둠) |
| `non-sync` | 구분선 | DOM 앵커가 존재하지 않으며, 블록이 순수 `<hr>`로 렌더링됨 |

`sync.js`는 `non-sync` 단독에만 작용합니다 — 알려진 다른 모든 역할은 "이 행이 스크롤 앵커 역할을 함"을 의미합니다. 4개 범위를 벗어난 역할은 대역 내 맵에서 거부되며 포워드 표류 하에서는 경고와 함께 앵커로 취급됩니다. 맵 대 DOM 표류 검사는 `non-sync` 행을 건너뛰는데, 이들 행에는 찾을 앵커가 정당하게 없기 때문입니다.

### `fallback_status`

이 블록의 콘텐츠에 발생한 결과. 동일한 값이 `data-fallback` DOM 속성이 됩니다.

| 값 | 의미 |
|---|---|
| `translated` | 프로바이더가 유닛에 대한 번역을 반환했고 검증을 통과함 |
| `preserved` | 콘텐츠가 의도적으로 그대로 유지됨 — 프로바이더가 유닛에 대해 그렇다고 답했거나, 블록이 전혀 배치 처리되지 않음 |
| `partially_translated` | 블록의 일부는 번역되었고 일부는 그렇지 않음 — 유닛에 대한 프로바이더 자체의 평결이거나, 상태가 불일치하는 행 윈도우로 분할된 크기 초과 테이블의 병합된 평결 (DCR-0026) |
| `fallback_source` | 블록의 바이트가 원문 바이트임: 재시도가 소진되었거나, 프로바이더가 옵트아웃했거나, html 텍스트 추출이 실패함 |

번역 유닛이 되지 않는 블록들 — 구분선, 이미지, `skipped` 노드, 및 추출 가능한 텍스트가 없는 html 블록 — 은 `preserved`를 전달하며, 이는 정직한 상태입니다: LLM이 이를 본 적이 없습니다. 추출이 실패한 html 블록은 `fallback_source`를 전달하고 이스케이프된 자리 표시자로 렌더링됩니다. 파이프라인이 상태를 전혀 확정하지 못한 *번역 가능한* 블록은 조용히 번역된 것으로 주장되는 대신 `fallback_source`로 기록됩니다; 그 조합은 내부 파이프라인 결함을 알립니다.

유닛이 각 상태에 도달하는 방법 — 계층화된 검증기, 제한된 재시도, 및 이를 따르는 폴백 — 은 [검증, 재시도 및 폴백 모델](../../../explanation/developer/ko/validation-retry-fallback-model.md)에 설명되어 있습니다.

### `source_range` 및 `target_range`

UTF-8 상의 반열린 `[start, end)` 바이트 범위로, `{ \"start\": <n>, \"end\": <n> }`로 직렬화됩니다. 이들은 문자나 UTF-16 인덱스가 아닌 바이트 오프셋입니다.

`source_range`는 **파싱된 형태의** 원문 문서를 인덱싱하며, `document_id`는 동일한 문자열을 해시합니다. 이것은 한 가지 예외를 제외하면 호출자가 전달한 파일과 바이트 단위로 동일합니다: NUL 바이트 (`U+0000`)는 파싱 전에 `U+FFFD`로 대체됩니다. CommonMark가 치환을 요구하고 파서가 그 이후의 위치만 보고하기 때문입니다. 원본 파일에서 `source_range`를 잘라내는 소비자는 먼저 동일한 치환을 적용해야 합니다; `out.md` 및 렌더링된 패널은 이미 치환된 형태를 전달합니다.

`target_range`는 재생성된 마크다운 — `out.md` — 을 인덱싱하며, 이는 문서의 어느 쪽에서도 NUL을 절대 포함하지 않습니다: 원문 측은 치환에 의해, 대상 측은 거부에 의해 포함하지 않는데, 번역된 페이로드 내부의 `U+0000`은 스키마 검증 계층을 실패시키기 때문입니다.

**비어 있는 범위 내 범위 (`0..0`)는 정당합니다**: `build_alignment_map`은 재생성 오프셋이 주어지지 않은 블록에 대해 이를 배출하고, 최대 8개 id를 지정하여 문서에 대한 `tracing::warn` 로그를 하나 남깁니다. 비어 있는 범위가 안전한 답변이므로 오류가 아닌 경고입니다 — 대안은 잘못된 문자열을 인덱싱하게 됩니다.

범위는 렌더러에 단지 조언만 하는 것이 아닙니다. `transync-syntax::render`는 잘라낼 수 없는 범위 — 뒤바뀜, 끝을 지남, 또는 문자 중간에 도달함 — 를 반복되는 `source_block_id`에 대한 `RenderError::DuplicateRow` 및 어떤 행도 이름을 지정하지 않는 문서 블록에 대한 `RenderError::UncoveredBlock`과 함께 `RenderError::UnusableRange`로 거부합니다. 각 패널은 자신이 잘라내는 대상에 대해 측정됩니다: 대상 패널의 범위는 번역된 마크다운에 대해 측정되는 행의 `target_range`인 반면, 원문 패널은 파싱된 문서 자체의 블록 범위를 원문 텍스트에 대해 측정합니다. 브라우저 데모에서 거부는 치명적인 `render failed: … unusable target byte range …`로 표출됩니다.

`web/js/wasm-demo.js`는 추가적으로 편집 모델을 생성하기 전에 `target_range`를 검사하며, 사용자가 편집하도록 허용할 행(non-`html`, non-`skipped`, non-`non-sync`)에 대해서만 검사합니다: 정수 오프셋, `0 <= start <= end`, `out.md` 내부의 `end`, 및 UTF-8 문자 경계 상의 두 끝점. 데모가 게이트 처리하지 않는 행은 렌더러 자체의 거부에 의해 커버됩니다.

### `parent_id`

항상 `null`. `child-only` `sync_role`과 함께 향후 중첩 앵커 체계를 위해 예약되어 있습니다. 파서는 잎-블록 방식이므로 어떤 블록도 지칭할 부모가 없으며 렌더링된 어떤 블록도 `data-parent-id` 속성을 전달하지 않습니다.

## `validation_summary`

| 필드 | 타입 | 값 |
|---|---|---|
| `total_units` | number (u32) | 블록이 번역 유닛이 된 행의 수. |
| `translated` | number (u32) | 그중 `translated`로 확정된 수. |
| `preserved` | number (u32) | …`preserved`로 확정된 수. |
| `partially_translated` | number (u32) | …`partially_translated`로 확정된 수. |
| `fallback_source` | number (u32) | …`fallback_source`로 확정된 수. |
| `retried_units` | number (u32) | 시도 로그가 1보다 큰 번호의 시도를 보유한 유닛의 수. |

집계는 **행이 아닌 유닛**을 카운트합니다. 구분선, 이미지, `skipped` 노드, 및 그 뒤에 유닛이 없는 html 블록은 `blocks`에 나타나므로 소비자가 이를 렌더링하고 앵커링할 수 있지만, 여기의 모든 카운터에서는 제외됩니다. 따라서 `block_kind: "html"` 행의 수가 카운트된 html 유닛 수를 초과할 수 있습니다; 카운트되지 않은 행은 검증 보고서의 경고 채널에 표기됩니다. 4개의 상태 카운터는 `total_units`를 분할(partition)합니다.

`retried_units`는 구문 계층이 계산할 수 없는 하나의 필드입니다. 파이프라인의 보고서 단계는 검증 보고서의 유닛별 시도 로그로부터 이를 패치하여 입력합니다: 유닛은 `attempt_number > 1`인 시도를 누적했을 때 재시도된 것으로 카운트됩니다. 시도 번호 0에서 기록된 캐시 히트는 카운트되지 않습니다.

CLI는 이러한 카운터를 직접 읽습니다: 실행은 `total_units > 0`이고 `fallback_source == total_units`일 때 `3`으로 종료하며, 출력은 여전히 작성됩니다. 종료 코드 및 `--verbose` 집계 줄은 [CLI 참조 문서](../../user/ko/cli.md)에 있습니다; 검증 보고서에 대비하여 실행 카운터를 읽는 것은 [번역 실행 진단](../../../how-to/user/ko/diagnose-a-translation-run.md)에서 다룹니다.

## 소비자가 받아야 하는 정보

두 기준 소비자가 동일한 파일로부터 서로 다른 것을 필요로 하기 때문에 두 가지 서로 다른 기준이 적용됩니다.

**Rust (`render_pair`, 또는 `AlignmentMap`으로의 모든 `serde_json` 읽기)** — 위에 문서화된 모든 필드가 올바른 타입과 함께 존재해야 합니다. serde 기본값이 없으므로 누락된 `blocks`, `source_block_id`가 없는 행, 잘못 입력된 오프셋 또는 알 수 없는 `sync_role`은 모두 역직렬화 시 이름과 함께 실패합니다. 알 수 없는 추가 필드는 무시됩니다.

**`web/js/sync.js`** — 행 게이트는 동기화에 필요한 정보만을 요구합니다:

- `blocks`는 배열임;
- 모든 행은 비어 있지 않은 문자열 `source_block_id`를 가진 객체임;
- `source_block_id`는 맵 전반에 걸쳐 유일함;
- `target_block_id`는 비어 있지 않은 문자열로 존재할 때 이것과 동일함;
- `sync_role`은 알려진 4개 값 중 하나의 문자열임.

범위, 순서, `block_kind` 및 `fallback_status`는 거기서 단속되지 않습니다. 행 게이트 옆에 마운트 시점의 두 가지 거부 사항이 위치합니다: 동기화 가능한 블록을 전혀 설명하지 않는 맵 — `blocks: []`, 또는 모든 행이 `non-sync` — 은 패널이 앵커를 전달할 때 거부됩니다 (빈 패널은 여전히 빈 맵을 마운트함), 그리고 `mountSync`는 두 패널 모두로 전달된 하나의 요소를 거부합니다. 모든 거부는 `null`을 반환하고 패널을 연결되지 않은 상태로 남겨두며 콘솔에 이유를 출력합니다.

게시된 맵을 소비하는 데모 번들은 [데모 번들 제공](../../../how-to/operator/ko/serve-the-demo-bundle.md)에 설명되어 있으며, 맵을 생성하는 브라우저 내 렌더러는 [wasm 데모 빌드](../../../how-to/operator/ko/build-the-wasm-demo.md)에 있습니다.

## 예시

설명 전용 — 실제 맵의 두 행:

```json
{
  "schema_version": "1.2.0",
  "document_id": "a91f2c0d2e1bbb40",
  "source_language": "en",
  "target_language": "ko",
  "detected_source_language": "en",
  "generator": { "name": "transync", "version": "0.3.0" },
  "blocks": [
    {
      "source_block_id": "h1-0001",
      "target_block_id": "h1-0001",
      "block_kind": "heading-1",
      "source_order": 0,
      "target_order": 0,
      "source_range": { "start": 0, "end": 18 },
      "target_range": { "start": 0, "end": 22 },
      "sync_role": "anchor",
      "fallback_status": "translated",
      "parent_id": null
    },
    {
      "source_block_id": "p-0002",
      "target_block_id": "p-0002",
      "block_kind": "paragraph",
      "source_order": 1,
      "target_order": 1,
      "source_range": { "start": 20, "end": 187 },
      "target_range": { "start": 24, "end": 211 },
      "sync_role": "anchor",
      "fallback_status": "fallback_source",
      "parent_id": null
    }
  ],
  "validation_summary": {
    "total_units": 2,
    "translated": 1,
    "preserved": 0,
    "partially_translated": 0,
    "fallback_source": 1,
    "retried_units": 0
  }
}
```

## 관련 문서

- [CLI 참조 문서](../../user/ko/cli.md) — 맵을 작성하는 플래그 및 이를 읽는 종료 코드.
- [`Translator` 트레이트](./translator-trait.md) — 각 행의 `fallback_status`가 되는 결과를 내는 프로바이더 계약.
- [검증, 재시도 및 폴백](../../../explanation/developer/ko/validation-retry-fallback-model.md) — 유닛이 각 상태에 도달하는 방법.
- [아키텍처 개요](../../../explanation/developer/ko/architecture-overview.md) — 파이프라인과 브라우저 사이에서 맵이 위치하는 곳.
