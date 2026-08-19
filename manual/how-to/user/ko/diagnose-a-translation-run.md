---
type: How-To Guide
title: 번역 실행 진단 방법
description: 종료 코드, stderr 라인, validation-report.json을 읽고 실행 실패, 거부, 폴백 발생 또는 비정상 동작 이유를 파악하고, 실제 실행 전에 프로필의 컴파일된 프롬프트를 검증하세요.
tags: [cli, troubleshooting, validation, SCN-07]
audience: user
language: ko
sources:
  - { id: en-source, resource: manual/how-to/user/en/diagnose-a-translation-run.md }
generated:
  by: antigravity/antigravity-cli
  at: 2026-08-19T00:25:02Z
synced_hash: bee0507015e2971fc118091c1e96f4eb90a1295e4ce9f3f57d3d775776629ded
---
# 번역 실행 진단 방법

`transync translate` 실행이 0이 아닌 코드로 종료되었거나, `0`으로 종료되었으나 예기치 않은 폴백, 나타나지 않는 용어집 용어, 반만 번역된 테이블 등 출력이 이상해 보이는 경우에 사용하세요.

## 1. 종료 코드를 먼저 확인하세요

| 종료 코드 | 의미 |
|---:|---|
| `0` | 성공. 성공적인 실행도 `transync: note:` 권고 줄을 출력할 수 있습니다 — 4단계를 참조하세요. |
| `1` | 인자 오류 — 아무것도 실행되지 않음. CLI 자체의 거부 조건(빈 언어 레이블, `--map` 없는 `--output`, 잘못된 형식의 `--base-url`, 누락된 API 키)도 포함됩니다. |
| `2` | 입력을 읽을 수 없음. 초과 크기의 `--input`, 비 UTF-8 입력, 초과 크기의 `--profile` 또는 `--system-prompt-file`, 파싱 시점의 블록 중첩 거부 조건도 포함됩니다. |
| `3` | 번역 가능한 모든 단위가 원문으로 폴백됨. 출력 파일은 작성되었습니다 — 2단계로 이동하세요. |
| `4` | 쓰기 실패 — 원자적 쓰기 또는 번들 쓰기 오류 발생. 번역이 아닌 파일시스템 문제입니다. |
| `5` | 잔여 오류: 분류되지 않은 번역기 오류, 잘못된 공급자 응답, 또는 `transync serve` 시 바인딩할 수 없는 주소. |
| `6` | 공급자가 **실행 설정 방식**(자격 증명, 존재하지 않는 `--model`, 너무 작은 출력 한도, 또는 컨텍스트 창 초과)으로 인해 거부함. 아무것도 게시되지 않았습니다. 지정된 설정 값을 수정한 후 재실행하세요. |
| `7` | 공급자가 **이 문서의 콘텐츠**(콘텐츠 필터링 또는 모델 거절)를 거부함. 아무것도 게시되지 않았으며, 변경 없이 재실행해도 해결되지 않습니다 — 재시도는 리터럴 재전송이므로 동일한 콘텐츠를 보냅니다. |

stderr 라인은 대부분의 문제에 대한 가장 빠른 진단 수단이지만, 거부 주체에 따라 출력되는 접두사가 달라집니다:

- `transync translate`는 `transync: <reason>`을 출력합니다 — `--quiet`로 억제 가능.
- 잘못된 플래그는 인자 파서에 의해 감지되며, `--quiet`로 억제할 수 없는 파서 자체 사용법 오류를 출력합니다 (파싱이 먼저 실행됨).
- `transync serve`는 `transync serve: <reason>`을 출력하며 `--quiet` 플래그를 **제공하지 않습니다**.

아무런 출력도 보이지 않는다면 cargo 자체의 컴파일 오류까지 가리는 `cargo run --quiet`로 실행했는지 확인하세요.

## 2. 단위별 리포트 확인

`--validation-report report.json`과 함께 재실행하세요. `--out-dir` 실행은 플래그 없이도 항상 대상 디렉터리에 `validation-report.json`을 작성합니다.

각 `per_unit[*]` 항목에는 모든 시도 내역이 포함되어 있습니다:

```json
{
  "unit_id": "t-0006",
  "attempts": [
    { "attempt_number": 1, "rejected_by": "per_kind_shape", "rejection_reason": "table column count 3 != 4" },
    { "attempt_number": 2, "rejected_by": "schema", "rejection_reason": "unit missing from batch response", "batch_fault": true },
    { "attempt_number": 3, "rejected_by": null, "rejection_reason": null }
  ],
  "final_status": "translated",
  "warnings": []
}
```

마지막 시도의 `rejected_by: null`은 해당 시도가 통과했음을 의미합니다; 중요한 것은 최종 결과인 `final_status`입니다. `final_status`가 `fallback_source`인 단위는 재시도 횟수를 모두 소진한 것이며, 마지막 시도의 `rejected_by`는 이를 계속 거부한 계층을 나타냅니다.

**`batch_fault`는 진단을 바꾸는 결정적 필드입니다.** 이 필드는 `true`일 때만 나타나며, 해당 단위 자체의 콘텐츠 문제 때문이 아니라 공급자가 배치 응답에서 이 단위의 행을 누락하거나 중복 출력하여 실패했음을 의미합니다. 이러한 시도는 단위의 콘텐츠 재시도 예산 대신 배치별 스키마 예산에서 차감됩니다. `batch_fault`를 동반한 `rejected_by: "schema"`는 공급자 출력 형태의 문제입니다; 반면 이 필드가 *없는* 동일한 값은 해당 단위 자체의 문제입니다.

### `rejected_by`가 나타낼 수 있는 값

| 값 | 계층 | 추정 원인 |
|---|---|---|
| `schema` | Structured-output 형태 | 응답이 스키마 요구대로 해당 단위의 행을 포함하지 않음 — 주로 거부 또는 누락/중복된 행 (`batch_fault` 확인). |
| `per_kind_shape` | 블록 종류별 형태 | 모델이 테이블 열 개수, 리스트 깊이, 코드 펜스 info 문자열, 또는 제목 수준을 변경함. |
| `fragment_reparse` | 프래그먼트 재파싱 | 반환된 텍스트가 GFM 하에서 동일한 블록 종류로 다시 파싱되지 않음. |
| `inline` | 인라인 보호 | 링크 또는 이미지 대상이 변경되었거나, 정책 대상 코드 스팬 검사에 실패함. 묵시적으로 손상된 링크를 잡아내는 계층입니다. |
| `provider` | 공급자 | 검증기가 결함을 발견한 것이 아니라 공급자 자체가 이 단위를 거부하거나 실패 처리함. |

와이어 어휘집에는 2가지 추가 값인 `id_set`과 `full_reparse`가 존재하지만, 단위별 시도에 이를 할당하는 코드 경로는 없습니다. 이 값들은 배치 및 문서 수준의 불변성을 보호하며 단위의 `rejected_by`가 아닌 실행 실패로 나타납니다. 이 파일에서 이 값들을 찾아다니지 마세요.

### 잘못 오인할 수 있는 2가지 사항

**캐시 적중은 거부가 아닙니다.** 승인된 캐시 적중은 `attempt_number: 0`, `rejected_by: null`, `rejection_reason: null`로 기록됩니다. 이전 가이드에서는 `rejection_reason`에서 `"cache hit"` 감시 문자열을 확인하도록 안내했으나, 사용자가 캐시 적중을 실패로 오인하지 않도록 해당 문자열이 의도적으로 제거되었습니다. *거부된* 캐시 적중은 실제 거부 사유를 유지합니다.

**`unit_id`는 블록이 아닌 행 윈도우(row window)를 가리킬 수 있습니다.** 초과 크기 테이블이 패킹 시 분할될 때 각 윈도우는 자체 ID를 가진 독립적 번역 단위가 되며, 재생성 전에 하나의 테이블로 재조합됩니다. 따라서 실패한 `unit_id`는 렌더링된 출력물에서 찾을 수 있는 블록이 아닌 테이블의 일부 슬라이스를 가리킬 수 있습니다 — 그리고 윈도우 일부가 실패한 테이블은 블록 수준에서 `partially_translated`로 보고되는 반면, *어떤* 윈도우가 폴백되었는지는 여기 리포트에만 나타납니다.

`total_retries`, `total_fallbacks`, `provider_retries`, `batch_schema_faults`는 서로 다른 예산에 대한 4개의 독립된 카운터입니다. 이들을 하나의 "재시도 횟수"로 합산하지 마세요.

## 3. stderr 라인을 알려진 원인과 대조

| 증상 | 원인 | 해결 방법 |
|---|---|---|
| `authentication: … does not have access to model …` (종료 코드 `6`) | 계정 등급이 설정된 모델에 접근할 수 없음. | 계정 등급이 지원하는 모델로 설정하거나, 모델/엔드포인트 조합이 잘못된 경우 디스패치 표면을 수정하세요. |
| `output ceiling exhausted: …` (종료 코드 `6`), 세부 정보에 `finish_reason: length` 또는 `reason: max_output_tokens`가 포함됨 | 모델이 작성해야 하는 내용에 비해 배치의 출력 한도가 너무 작았음. 설계상 치명적 오류 — 변경 없이 재실행해도 재현됨. | `--target-output-tokens`를 높이세요; 기존에 낮췄다면 `--output-expansion-factor`를 높이세요 (절대 낮추지 마세요). 테이블의 경우 `--table-strategy row-window-first`가 헤더 포함 윈도우로 분할합니다 — 이는 이미 기본 설정이므로 사용자 지정 프로필이 이를 껐는지 확인하세요. |
| `context window exceeded: …` (종료 코드 `6`) | 요청이 모델의 입력 컨텍스트 창을 초과함. | `--target-input-tokens-per-batch` 또는 `--max-units-per-batch`를 낮추거나 더 큰 컨텍스트 창을 지원하는 모델을 사용하세요. |
| `content filtered: …` / `model refused: …` (종료 코드 `7`) | 공급자가 이 콘텐츠의 번역을 거부함. 설정 오류가 아님 — 어떤 플래그로도 결과를 바꿀 수 없음. | 문서를 건너뛰거나 거부되는 블록을 격리하여 다른 문장으로 수정하세요. 변경 없이 재실행하지 마세요. |
| `model output_text was not valid JSON` (종료 코드 `5`) | 모델이 엄격한 Structured Outputs를 지원하지 않거나 스키마 대신 일반 텍스트를 출력함, 또는 프록시가 응답 형식을 제거함. | Structured Outputs를 지원하는 모델을 사용하세요; 프록시 문제를 배제하기 위해 공급자에 직접 테스트하세요. |
| `every translatable unit fell back to source (N/N)` (종료 코드 `3`) | 모든 단위가 검증 재시도를 소진함. | 2단계로 이동하세요 — 이 메시지 자체는 원인을 나타내지 않습니다. |

동기화 멈춤 및 번역 속도 저하 진단을 포함한 전체 증상별 목록은 프로젝트의 `docs/Troubleshooting.md`에 수록되어 있습니다.

## 4. 성공적인 실행 시 권고 안내 줄 확인

종료 코드 `0`으로 성공한 실행도 stderr에 `transync: note:` 라인을 출력할 수 있습니다. 이는 게시 과정에서 결정된 사항이나 우회 처리된 내용을 보고하는 것으로, 실패가 아닙니다. 단순 정보로 참고하세요; 이 메시지가 나타났다고 해서 실행이 고장 난 것은 아닙니다.

`--verbose`는 더 많은 정보를 추가합니다: 단계별 파이프라인 진단, 검증 집계, 그리고 자동 용어집 사전 점검이 실행된 경우 해당 결과. `--verbose`와 `--quiet`는 상호 배타적이며 파싱 시점에 충돌을 일으킵니다.

## 5. 프로필의 컴파일된 프롬프트를 대외적으로 검증

용어집 용어나 제약 조건이 모델에 전달되지 않는 것 같다면, API 호출 비용을 지불하기전에 실제 전송될 프롬프트를 렌더링해 보세요:

```rust
use transync::profile::{load_profile, render_prompt_body};

let profile = load_profile("profile.toml")?;
let compiled = render_prompt_body(&profile, "en", "ko");
println!("{}", compiled.prompt_body);
```

`render_prompt_body`는 자체 출력에 대해 멱등성(idempotent)을 가지므로 두 번 호출하더라도 덧붙여진 섹션이 중복되지 않습니다 — 여기서 출력되는 내용이 실제 실행 시 컴파일되는 프롬프트입니다.

알아두어야 할 한 가지 제한 사항: 이 함수는 프로필의 **전체** 용어집을 렌더링합니다. 항목에 섹션 범위가 설정되어 있다면 섹션별 필터링은 나중에 패커(batcher)에서 이루어지므로, 여기서 출력되는 내용은 개별 배치가 실제로 전달하는 내용의 상위 집합입니다. [용어집 항목이 해석되는 방식](../../../explanation/user/ko/how-glossary-entries-are-resolved.md)을 참조하세요.

## 관련 문서

- [CLI를 사용하여 문서를 번역하는 방법](./translate-a-document-with-the-cli.md)
- [프로필 TOML을 작성하는 방법](./write-a-translation-profile.md)
- [실행 간 웜 캐시(warm cache)를 재사용하는 방법](./reuse-a-warm-cache-across-runs.md)
- [CLI 플래그](../../../reference/user/ko/cli.md) — 모든 플래그, 종료 코드, 및 환경 변수
- [`Translator` 트레이트](../../../reference/developer/ko/translator-trait.md) — 공급자 오류 배리언트 및 안정적 코드
- [계층화된 검증 및 제한된 재시도/폴백 구조의 이유](../../../explanation/developer/ko/validation-retry-fallback-model.md)
