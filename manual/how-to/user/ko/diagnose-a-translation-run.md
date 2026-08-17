---
type: How-To Guide
title: 번역 실행 진단 방법
description: 종료 코드(non-zero) 및 validation-report.json을 읽어 실행이 실패했거나, 폴백(fallback)되었거나, 결과가 잘못된 이유를 찾고 실제 실행 전에 프로필의 컴파일된 프롬프트를 검증합니다.
tags: [cli, troubleshooting, validation]
audience: user
language: ko
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:02Z
sources:
  - { id: en-source, resource: manual/how-to/user/en/diagnose-a-translation-run.md }
synced_hash: 32df5bb457ff75de6ca03e0f169146be0c3e01c6603e8f07017b46a2d7120d39
---
# 번역 실행 진단 방법

`transync translate` 실행이 0이 아닌 코드(non-zero)로 종료되었거나, `0`으로 종료되었지만 출력 결과가 이상해 보일 때(예상치 못한 폴백 발생, 용어집 (Glossary) 단어가 적용되지 않음 등) 이 가이드를 참조하십시오.

## 1. 종료 코드 먼저 확인하기

| Exit | 의미 및 해석 |
|---:|---|
| `1` | 인자 오류 — 호출 명령을 수정하십시오. 실행된 항목이 없습니다. |
| `2` | `--input` 경로가 존재하지 않거나 읽을 수 없습니다. |
| `3` | 모든 번역 대상 단위가 원문으로 폴백(fallback)되었습니다. 출력 파일은 *작성되었으므로* step 2로 이동하십시오. |
| `4` | 쓰기 실패 — 원자적 쓰기(atomic-write) 또는 번들 쓰기에서 오류가 발생했습니다. 번역이 아닌 파일 시스템의 문제입니다. |
| `5` | 기타 나머지 — 분류되지 않은 번역기 (Translator) 오류, 손상된 제공자(provider) 응답, 또는 주소 바인딩에 실패한 `transync serve`. 대부분의 `5`번 오류는 stderr의 `transync: <reason>` 줄에 진단 내용이 전체 표시됩니다. 무엇보다 먼저 이 메시지를 읽으십시오. |
| `6` | 제공자가 **설정 방식** 문제로 실행을 거부했습니다 — 자격 증명, 존재하지 않는 `--model`, 너무 작은 `[batching].target_output_tokens`, 또는 컨텍스트 윈도우 초과. 아무것도 게시되지 않았습니다. 지목된 설정 항목을 수정하고 다시 실행하십시오. stderr 줄에 원인이 표시됩니다. |
| `7` | 제공자가 **이 문서의 콘텐츠**를 거부했습니다 (`content filtered` 또는 `model refused`). 아무것도 게시되지 않았으며, 다시 실행해도 변경되지 않습니다 (재시도는 축자 재제출이므로 동일한 콘텐츠를 보냅니다). 해당 문서를 건너뛰거나 거부된 부분을 분리하십시오. |

`--quiet`를 전달하지 않았다면 0이 아닌 모든 종료 시 stderr에 `transync: <reason>` 줄이 출력됩니다. 출력이 *전혀* 보이지 않는다면 `cargo run --quiet` 환경에서 실행했을 가능성이 높으며, 이는 cargo 자체의 컴파일 오류도 가립니다 — `--quiet`를 제거하고 다시 실행하십시오.

## 2. 단위별 보고서 받기

`--validation-report report.json` 플래그를 추가하여 다시 실행하십시오 (`--out-dir` 실행은 항상 플래그 없이도 대상 디렉터리에 `validation-report.json`을 작성합니다). 각 `per_unit[*]` 항목에는 모든 시도가 기록되어 있습니다:

```json
{
  "unit_id": "t-0006",
  "attempts": [
    { "attempt_number": 1, "rejected_by": "per_kind_shape", "rejection_reason": "table column count 3 != 4" },
    { "attempt_number": 2, "rejected_by": null, "rejection_reason": null }
  ],
  "final_status": "translated",
  "warnings": []
}
```

마지막 시도에서 `rejected_by: null`은 해당 시도가 통과했음을 의미합니다 — 결과 결정에 중요한 것은 `final_status`입니다. `final_status: "fallback_source"`인 단위는 재시도 횟수를 모두 소진한 것이며, 마지막 시도의 `rejected_by`는 지속적으로 거부한 레이어를 지목합니다.

| `rejected_by` | 유력한 원인 |
|---|---|
| `schema` | 모델이 잘못된 unit-id 세트를 반환함 — 대부분 모델 거부 반응입니다. |
| `per_kind_shape` | 모델이 테이블 열 개수, 리스트 깊이, 코드 펜스 정보, 또는 헤딩 레벨을 변경했습니다. |
| `fragment_reparse` | 모델이 동일한 블록 (Block) 종류로 다시 파싱되지 않는 텍스트를 내보냈습니다 — Structured Outputs를 지원하는 모델에서는 드물게 발생합니다. |

`total_retries`, `total_fallbacks`, `provider_retries`, `batch_schema_faults`는 4개의 별개 카운터이므로 하나의 "재시도 횟수"로 합산하지 마십시오. 캐시 히트(cache hit)는 `attempt_number: 0`, `rejection_reason: "cache hit"`와 같이 바이패스된 별도의 비용 0인 시도로 표시됩니다.

## 3. Stderr 줄을 알려진 원인과 매칭하기

| 증상 | 원인 | 해결책 |
|---|---|---|
| `authentication: … does not have access to model gpt-…` (종료 코드 `6`) | 사용 중인 계정 티어에서 설정된 모델에 접근할 수 없습니다. | `TRANSYNC_OPENAI_MODEL=gpt-4o-2024-08-06`(또는 사용자 계정 티어에서 사용 가능한 다른 모델)으로 재정의하거나, 모델/엔드포인트 조합이 잘못된 것 같으면 `TRANSYNC_OPENAI_API=chat`/`responses`로 디스패치 서피스를 강제 지정하십시오. |
| `model output_text was not valid JSON` (`MalformedResponse`, 종료 코드 `5`) | 모델이 엄격한 Structured Outputs를 지원하지 않거나, 스키마 대신 줄글 거부 메시지를 작성했거나, 프록시가 `response_format`을 제거했습니다. | Structured Outputs 지원 모델로 전환하십시오. 모호한 시스템 프록시/프롬프트 문구를 수정하십시오. 프록시 문제를 배제하기 위해 `https://api.openai.com`에 직접 테스트해 보십시오. |
| `output incomplete (reason: max_output_tokens)` / `(finish_reason: length)` (종료 코드 `6`) | 배치 (batch)의 출력 한도(`[batching].target_output_tokens` / `--target-output-tokens`)가 모델이 작성해야 하는 분량에 비해 너무 작았습니다. 의도적인 종결이므로 변경 없이 재실행하면 재현됩니다. | `--target-output-tokens` 값을 늘리십시오. `--output-expansion-factor`를 줄였었다면 다시 높이십시오(절대 낮추지 마십시오). 최후의 수단으로 크기가 너무 큰 소스 블록 (Block)을 분할하십시오. 온난 캐시(warm cache)는 이 세 수정 사항 이후에도 유효합니다. 출력 한도는 캐시 키 축이 아닙니다. |
| `content filtered: …` / `model refused: …` (종료 코드 `7`) | 제공자가 이 문서를 번역하지 않으려 함: 콘텐츠 정책으로 작성이 중단되었거나 모델이 거부 메시지를 작성했습니다. 설정 오류가 아니므로 플래그 변경으로 해결되지 않습니다. | 문서를 건너뛰거나 거부된 블록 (Block)을 분리하여 문구를 수정하십시오. 변경 없이 다시 실행하지 마십시오. 재시도는 축자 재제출이므로 동일한 콘텐츠가 보내집니다. |
| `every translatable unit fell back to source (N/N)` (종료 코드 `3`) | 모든 단위의 검증 (Validation) 재시도가 소진되었습니다. | step 2로 돌아가 `per_unit[*]`을 읽으십시오. 이 메시지 자체만으로는 원인을 알려주지 않습니다. |

동기화 멈춤(sync-freeze) 및 느린 실행 진단을 포함하여 증상별 전체 목록은 `docs/Troubleshooting.md`에 있습니다.

## 4. 프로필의 컴파일된 프롬프트를 대역 외(out-of-band)로 검증하기

용어집 (Glossary) 단어나 제약 조건이 모델에 전달되지 않는 것 같으면 실제 API 호출 비용을 들이기 전에 실행 시 실제로 전송될 프롬프트를 렌더링해 보십시오:

```rust
use transync::profile::{load_profile, render_prompt_body};

let profile = load_profile("profile.toml")?;
let compiled = render_prompt_body(&profile, "en", "ko");
println!("{}", compiled.prompt_body);
```

`render_prompt_body`는 자체 출력에 대해 멱등성(idempotent)을 가지므로 두 번 호출해도 덧붙여진 `[constraints]`/용어집 섹션이 중복 생성되지 않습니다. 출력되는 내용은 실제 실행 시 컴파일되는 내용과 정확히 일치합니다.

## 관련 항목

- [CLI로 문서 번역하는 방법](./translate-a-document-with-the-cli.md)
- [번역 프로필 TOML 작성 방법](./write-a-translation-profile.md)
- [CLI 플래그](../../../reference/user/en/cli.md)
