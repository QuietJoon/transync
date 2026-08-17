---
type: Reference
title: "`Translator` 트레이트"
description: 메서드 시그니처, 안정적인 코드를 포함한 완전한 `TranslatorError` 변체 세트, 그리고 모든 구현체가 충족해야 하는 동작 계약.
tags: [providers, llm, reference, cancellation, DCR-0024, DCR-0029]
audience: developer
language: ko
sources:
  - { id: en-source, resource: manual/reference/developer/en/translator-trait.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:02Z
synced_hash: f06628d1b99a634dab7d1c3e8fa6c0a4d1a7edc7db13f3a12018cb5253316098
---
# `Translator` 트레이트

`transync-core::llm`에 선언되어 있고 파사드를 통해 다시 내보내지는 `transync::llm::Translator`입니다. 이는 `0.1.0` 이후 제공자 경계 역할을 해왔으며 해당 진화는 동결되기보다는 관리됩니다 — [트레이트 안정성](#trait-stability)을 참조하십시오. 이 트레이트는 `Send + Sync`이며, `translate` / `translate_with_cache`는 `T: Translator + ?Sized`를 사용하므로 `&dyn Translator`와 구체적인 타입이 모두 수락됩니다.

이 워크스페이스에는 두 가지 구현체가 출하됩니다: `transync-openai` (`TransyncOpenAI`) 및 `transync-anthropic` (`TransyncAnthropic`). 둘 다 라이브러리이며, 첫 번째만 번들로 제공되는 CLI에서 도달할 수 있습니다.

## 메서드

```rust
#[async_trait::async_trait]
pub trait Translator: Send + Sync {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError>;

    fn fingerprint(&self) -> ProviderFingerprint {
        ProviderFingerprint::from_type_name(std::any::type_name::<Self>())
    }

    fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        None
    }

    async fn extract_glossary(
        &self,
        req: &GlossaryExtractionRequest,
        cancel: &CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        let _ = (req, cancel);
        Ok(None)
    }
}
```

| 메서드 | 필수 여부 | 기본 동작 |
|---|---|---|
| `translate_batch` | 예 | — |
| `fingerprint` | 아니요 | `std::any::type_name::<Self>()`에서 파생됨 — 해당 타입의 모든 인스턴스가 상호 교환 가능한 경우에만 올바름. |
| `tokenizer_hint` | 아니요 | `None` — `TranslateOptions::model_id` 상의 OpenAI-모델-이름 휴리스틱으로 폴백함. |
| `extract_glossary` | 아니요 | `Ok(None)` — "지원되지 않음." |

## `cancel` 파라미터

`CancellationToken`은 `tokio_util::sync::CancellationToken`이며, `transync::CancellationToken`으로 다시 내보내집니다. 이는 파사드가 큐레이팅된 표면에 수락하는 유일한 외부 타입이며 의도된 사항입니다: 이 타입을 명시함으로써 소비자의 자체 의존성이 통합되어야 하는 `tokio-util` 버전(워크스페이스 매니페스트에서 `0.7`, `default-features = false`)도 고정됩니다.

이 토큰은 호출이 아닌 **실행(run)**의 토큰입니다 — `translate` / `translate_with_cache` 호출당 하나의 토큰이 사용되며, 모든 배치가 공유하고 모든 제공자 호출에 전달됩니다. 그 출처는 `TranslateOptions::cancel: Option<CancellationToken>`이며 기본값은 `None`입니다. `None`으로 실행되는 작업은 취소될 수 없으며 필드가 존재하기 전과 동일하게 동작합니다.

파이프라인은 모든 제공자 호출을 동일한 토큰과 경쟁시키며 (`pipeline::dispatch::translate_with_provider_retries`, `biased` `tokio::select!`) 패배한 쪽을 삭제하므로, 인자를 무시하는 구현체도 퓨처 삭제(future-drop)를 통해 계속 취소됩니다. 인자를 준수함으로써 얻는 추가적인 이점은 타입 지정된 응답, 드롭 취소(drop-cancellable)가 불가능한 작업에 대한 올바른 처리, 그리고 파이프라인을 거치지 않고 구현체가 직접 구동될 때의 올바른 처리입니다. 의무사항은 [동작 계약](#behavior-contract) 아래에 나열되어 있습니다.

토큰은 캐시 정체성 축이 아닙니다: 완료된 제공자 호출이 반환하는 내용을 변경할 수 없으며, 단지 호출이 발생하는지 여부만 결정합니다.

## 트레이트 안정성

변체 및 필드 추가는 `#[non_exhaustive]`에 의해 관리됩니다 (아래 오류 테이블 및 `contracts.md` §0 참조). 메서드 레벨의 진화는 더 좁으며, 두 가지 위험 클래스가 모두 발생했습니다:

- **추가적 (Additive).** 기본값을 전달하는 새 메서드는 기존의 모든 구현체가 계속 컴파일되도록 유지하고 문서화된 폴백 동작을 제공합니다. `fingerprint()` (승인된 v0.2 브레이킹 변경, DCR-0009), `tokenizer_hint()` 및 `extract_glossary()` (v0.2, DCR-0014 / DCR-0015)는 각각 이러한 방식으로 반영되었습니다.
- **하드 브레이킹 (Hard break).** 기존 메서드의 *시그니처* 변경은 기본값으로 흡수될 수 없습니다. `translate_batch` 및 `extract_glossary`에 추가된 `cancel` 파라미터는 승인된 v0.4 브레이킹 변경입니다 (DCR-0024): 모든 구현체는 다시 컴파일되기 전에 파라미터를 추가해야 합니다. `contracts.md` §5b에는 추가적이었을 대안(구 메서드로 위임하는 기본 `translate_batch_cancellable`)이 거부된 이유가 기록되어 있습니다: 하나의 작업에 두 메서드가 남게 되고, 구 메서드만 재정의하는 구현체는 소리 없이 취소 불가능한 상태로 남기 때문입니다.

작성 시점에 워크스페이스 버전은 `0.3.0`이며 `cancel` 파라미터는 `contracts.md`에서 v0.4.0 브레이킹으로 지정한 미릴리스 창에 위치합니다.

## `fingerprint()` — 캐시 네임스페이스 정체성

`ProviderFingerprint::new(family, parts)`는 단사적으로(injectively) 구성됩니다: 모든 부분(패밀리 레이블 포함)은 해당 바이트 길이, `\u{1F}` 구분 기호, 그다음 바이트로 프레이밍되므로, 부분 *콘텐츠*의 어떤 배치도 다른 *부분* 배치와 충돌할 수 없습니다. `from_type_name(name)`은 정확히 `new("type", &[name])`입니다.

규칙: **제공자가 반환하는 내용을 변경할 수 있는 모든 축** — 모델, 엔드포인트, API 표면, 프롬프트 템플릿 — 은 반드시 적용되어야 합니다. 과도한 구분(출력을 실제로 변경하지 않는 축)은 캐시 적중 대상에 대해 불필요한 재번역 비용을 발생시킵니다. 불충분한 구분은 공유 `Cache`가 한 구성의 출력을 다른 구성에 대해 재플레이하도록 만듭니다. 콘텐츠보다는 *지연 시간*(타임아웃)만을 제한하는 축은 올바르게 제외됩니다.

## `tokenizer_hint()` — 배치 예산 추정

`TokenizerHint`는 현재 두 가지 변체를 가진 `#[non_exhaustive]`입니다: `O200kBase`, `Cl100kBase`. 캐시 정체성 축이 아닙니다 — 배치 처리 형상은 추정 전용입니다. 모델 이름이 OpenAI 형상이 아닌 제공자는 엔진이 인식할 수 없는 이름에서 잘못 추정하도록 두기보다는 이를 재정의해야 합니다(SHOULD).

## `extract_glossary()` — 선택적 후보 용어집 사전 점검

호출자나 프로필이 선택한 경우에만 실행당 최대 한 번 호출됩니다. `req.source_text`는 신뢰할 수 없는 문서 데이터이며 지시문이 아닌 데이터로 프레이밍되어야 합니다(MUST). 반환된 `GlossaryEntry.scope` 값은 무시됩니다 — 병합 시 `GlobalAcrossDocument`가 강제됩니다. `Ok(None)` (지원되지 않음)과 `Ok(Some(vec![]))` (지원되지만 발견된 내용 없음)은 의도적으로 구별됩니다.

이 호출의 두 가지 결과는 다르게 취급되며, 이러한 비대칭성은 계약의 일부입니다:

- **오류**는 실행을 종료시키지 않습니다. 이는 `ValidationReport.auto_glossary`에 기록되며 실행은 정적 용어집만으로 진행됩니다. 배치가 디스패치되지 않았으므로 출력 커버리지에는 영향을 주지 않으며, 저하된 상태는 옵트아웃 기준선과 동일합니다. 보고서가 이에 대한 유일한 채널입니다 — 여기서의 `Err(_)`는 `tracing` 기록을 받지 않습니다.
- **취소**는 저하되어 사라지지 않습니다. 파이프라인은 이 호출이 반환된 후 실행 토큰을 다시 읽고, 메서드가 무엇으로 응답했든 `TransyncError::Cancelled`로 중단됩니다. 이러한 재읽기가 없다면 배치 처리 전 실행하는 단일 호출 중 취소하는 것이 "정적 용어집으로 진행됨"으로 해석되어 전체 문서가 여전히 디스패치될 것입니다.

## `TranslatorError` — `#[non_exhaustive]`, 13개 변체

각 변체는 호출자가 무엇을 해야 하는지가 아니라 **제공자가 중단된 이유**를 명시합니다. 재시도 가능성은 분류 체계 상의 정책이며 체계 자체의 일부가 아닙니다; 파이프라인의 정책은 이 테이블의 한 열이며 애플리케이션의 사용자 방향 정책은 별개입니다.

| 변체 | 의미 | 파이프라인에 의해 재시도되는가? | 안정적인 코드 |
|---|---|---|---|
| `Network(String)` | 전송 레벨 실패. | 예 — 제한된 일시적 전송 예산. | `provider_network` |
| `Authentication(String)` | 자격 증명 실패. | 아니요 — 실행을 중단함. | `provider_auth` |
| `RateLimited { retry_after: Option<Duration> }` | 제공자 속도 제한. | 예 — 동일한 예산; 존재할 경우 `retry_after`를 준수하며 30초로 상한 지정됨. 없는 경우 지수 백오프 (200ms 두 배 증가, 5초로 상한 지정됨). | `provider_rate_limited` |
| `MalformedResponse(String)` | 구현체가 파싱할 수 없는 데이터를 제공자가 반환함. | 아니요 — 실행을 중단함. | `provider_malformed_response` |
| `Unsupported(String)` | 제공자가 배치의 제약 조건을 지원하지 않음. | 아니요 — 실행을 중단함. | `provider_unsupported` |
| `ContentFiltered(String)` | 제공자 자체의 콘텐츠 정책으로 인해 생성이 종료됨. 모델의 거부가 아니며 전송 오류도 아님; 요청의 어떤 것도 이를 변경할 수 없으므로 제공할 시정 조치가 없음. | 아니요 — 실행을 중단함. | `provider_content_filtered` |
| `OutputCeilingExhausted(String)` | 응답이 완료되기 전에 출력-토큰 상한이 소진됨. 상한은 요청에 포함되므로 `[batching].target_output_tokens`가 시정 조치임. | 아니요 — 실행을 중단함. | `provider_output_ceiling_exhausted` |
| `ContextWindowExceeded(String)` | 요청이 모델의 컨텍스트 윈도우(출력만이 아닌 입력 플러스 요청된 출력)에 맞지 않음. 시정 조치는 출력 상한이 아니라 배치 처리 구성(`[batching]` 토큰 예산 및 `max_units_per_batch`)임. v0.4.0에서 추가됨 (DCR-0029). | 아니요 — 실행을 중단함. | `provider_context_window_exceeded` |
| `ModelRefused(String)` | 모델이 거부하고 이를 언급함; 제공자의 거부 텍스트(어댑터에 의해 길이가 제한됨)를 전달하거나 거부 텍스트 없이 도착한 경우 고정 대리 텍스트를 전달함. | 아니요 — 실행을 중단함. | `provider_model_refused` |
| `ResponseTooLarge(String)` | 응답 본문이 어댑터의 크기 캡을 초과하여 읽히지 않음. | 아니요 — 실행을 중단함. | `provider_response_too_large` |
| `ProviderRejected { status: Option<u16>, message: String }` | 제공자가 응답에 실패하기보다는 *요청*을 거부함. `status`는 제공자가 HTTP를 사용하는 경우 HTTP 상태이고 해당 코드가 없는 경우 `None`임. | 아니요 — 실행을 중단함. | `provider_rejected` |
| `Cancelled` | 호출이 취소되었기 때문에 중단됨: 실행 토큰이 발화되었거나 구현체가 자체 취소 소스를 가지고 있음. 의도적으로 메시지를 전달하지 않음. | 아니요 — 결코 다시 디스패치되지 않음; 그대로 재제출하는 것은 호출자가 중단하도록 요청한 바로 그 작업임. | `provider_cancelled` |
| `Other(String)` | 이 분류 체계가 명시하지 않는 제공자 실패에 대한 기존 포괄 항목(catch-all). | 아니요 — 실행을 중단함. | `provider_error` |

`OutputCeilingExhausted` 및 `ContextWindowExceeded`는 운영자가 혼동해서는 안 되는 쌍입니다: 둘 다 치명적이고 토큰에 관한 것이지만 **반대되는 노브(knob)**를 지정합니다. 정확히 그러한 이유로 개별 변체로 존재합니다. 모든 제공자가 `ContextWindowExceeded`를 자체 신호로 발생시키는 것은 아닙니다 — 너무 긴 요청에 HTTP 400으로 응답하는 제공자는 상태 기반의 `ProviderRejected` 분류를 유지하며 이는 거기서 솔직한 표현입니다. 두 개의 번들 어댑터 중 `transync-anthropic`만 제공자의 `model_context_window_exceeded` 정지 사유로부터 이를 생성하며, `transync-openai`에는 이에 매핑되는 신호가 없습니다.

`RateLimited.retry_after`는 `Option`입니다: `None`은 구현체가 사용할 수 있는 힌트를 제공자가 제공하지 않았음을 의미하며 파이프라인은 자체 지수 일정으로 폴백합니다. 번들로 제공되는 두 어댑터는 동일한 방식으로 헤더를 파싱합니다(델타-초, 및 세 가지 RFC 7231 날짜 형식 모두). 그리고 시스템 시계와 같거나 이전의 데드라인을 0 지연보다는 `None`으로 해소하므로, 뒤틀린 시계가 즉각적인 재디스패치로 이어지지 않습니다. 트리가 아닌 구현체가 전장에 배치하는 것은 자체 결정 사항입니다; 위의 30초 상한은 도착하는 무엇에나 적용됩니다.

`Other`는 의미가 좁아졌습니다. v0.3.0까지는 콘텐츠 필터, 소진된 출력 상한, 모델 거부, 크기 초과 본문, 거부된 요청이라는 5가지 구별되는 치명적 사유를 전달했으며, 명시적으로 인터페이스가 아닌 메시지 문자열을 일치시킴으로써만 분리할 수 있었습니다. 이제 해당 5가지에는 이름이 부여되었으며 `Other`는 *분류되지 않은* 제공자 실패만을 의미합니다. 이는 의도적으로 유지되므로 새로운 제공자 특이점이 변체를 기다리기보다는 메시지로 착륙할 수 있습니다.

`#[non_exhaustive]`: 변체 추가는 속성을 깨뜨리지 않는(non-breaking) 변경입니다; 소비자는 와일드카드(`_`) 매치 암을 반드시 유지해야 하며 컴파일 시점에 새 변체를 포착하기 위해 완전성(exhaustiveness)에 의존할 수 없습니다. 변체 제거 및 이름 변경은 계속 하드 브레이킹 변경입니다.

재시도되지 않는 모든 변체는 **치명적(terminal)**입니다: 한 번 직면하면 (또는 `Network`/`RateLimited`에 대한 일시적 재시도 예산(기본값 `max_per_batch_provider_retries = 1`, 전체 재시도 사다리에 걸쳐 배치당 청구됨)이 소진되면) 전체 파이프라인 실행이 `Err`로 중단되며, `fallback_source` 출력으로 저하되지 않습니다. 이것이 누락이 아닌 의도적인 설계 선택인 이유에 대해서는 [계층화된 검증 및 제한된 재시도/폴백 모델이 필요한 이유](../../../explanation/developer/ko/validation-retry-fallback-model.md)를 참조하십시오.

## `stable_code()` — 와이어 어휘

`TranslatorError::stable_code(&self) -> &'static str`은 실패 원인의 기계 읽기 가능한 이름을 반환합니다. 이 문자열들은 프로세스를 이탈합니다 — JSON 소비 또는 셸 소비 호출자가 키로 사용하는 것이므로 디버그 편의성이 아닌 와이어 어휘입니다. 라이브러리의 두 `stable_code` 매치는 **와일드카드 암 없이** 완전하므로, 코드가 할당되지 않고는 새 변체가 착륙할 수 없습니다.

`TransyncError::stable_code()`는 동일한 네임스페이스를 공유하며 라이브러리가 생성할 수 있는 모든 실패에 대해 응답합니다. 해당 `Translator` 암은 전체 제공자 패밀리를 하나의 문자열로 평탄화하기보다는 `TranslatorError::stable_code()`로 **위임**합니다. 전체 세트는 20개 코드입니다.

엔진 측, `TransyncError` 자체의 변체로부터:

| 코드 | 변체 |
|---|---|
| `parse_failed` | `Parse` |
| `validation_failed` | `Validation` |
| `regen_failed` | `Regen` |
| `profile_failed` | `Profile` |
| `alignment_failed` | `Alignment` |
| `cancelled` | `Cancelled` |
| `internal` | `Internal` |

제공자 측: 위의 변체 테이블에 있는 13개 `TranslatorError` 코드는 `TranslatorError::stable_code()` 및 `Translator` 변체를 통해 `TransyncError::stable_code()`에 의해 모두 반환됩니다.

어휘를 관리하는 규칙:

- 코드 문자열은 의미를 변경하지 않으며, 이름이 변경되거나 다른 용도로 변경되지 않습니다. 매핑은 **추가 전용(append-only)**입니다.
- 두 열거형 중 하나에 추가되는 새 변체는 기존 코드의 재사용보다는 **새로운** 코드를 필요로 합니다.
- 변체 추가가 속성을 깨뜨리지 않는 변경이므로 소비자는 인식되지 않은 코드를 불투명한 실패로 용인해야 합니다(MUST).

의미 축소는 추가 전용 규칙에 승인된 유일한 예외이며 v0.4.0 창을 이용합니다: `provider_error`는 여전히 "제공자 실패"를 의미하지만 이제는 *분류되지 않은* 실패만을 의미합니다. `provider_error`를 "모든 제공자 실패"로 읽었던 코드는 이제 인식할 수 없지만 불투명하게 취급해야 하는 12개의 형제 코드를 보게 됩니다.

`cancelled` 및 `provider_cancelled`는 의도적으로 하나의 코드가 아닌 두 개의 코드입니다. `cancelled`는 엔진의 응답입니다 — 호출자의 토큰이 발화했기 때문에 *실행*이 중단됨 — 파이프라인이 제공자 코드가 표면화되기 전에 토큰으로부터 취소를 결정하기 때문에 토큰 취소 실행에 대해 `translate` / `translate_with_cache`가 반환하는 유일한 값입니다. `provider_cancelled`는 `Translator` 호출이 중단되었음을 의미하며 정확히 하나의 경로를 통해 파이프라인 호출자에 도달합니다: 실행 토큰이 발화하지 않은 동안 구현체가 자체 취소 소스로부터 `TranslatorError::Cancelled`로 응답하는 경우입니다. 이는 `Translator(Cancelled)`로 표면화되는 다른 모든 것과 마찬가지로 치명적인 제공자 오류입니다. 트레이트를 직접 구동하는 것이 이를 확인하는 다른 방법입니다.

`docs/architecture/contracts.md` §1의 어휘 테이블은 해당 섹션을 긁어모아 두 메서드가 실제로 반환하는 코드와 디프를 수행하는 `crates/transync/tests/error_taxonomy.rs`에 의해 코드에 용접되어 있습니다. 해당 테스트는 본 매뉴얼 페이지가 아니라 계약 문서를 읽습니다.

## 동작 계약

모든 `translate_batch` 구현체는 다음 사항을 모두 충족해야 합니다(MUST). 파이프라인은 이들을 정당한 것으로 가정합니다; 위반 시 출력이 손상되거나 파이프라인이 정직하게 할당한 재시도/폴백 슬롯을 낭비하게 됩니다.

- 호출당 `batch.units`의 모든 유닛을 정확히 한 번 시도해야 합니다(SHOULD).
- 처리할 수 없는 모든 유닛에 대해 `UnitResult { output_kind: FailedNeedsFallback }`을 반환할 수 있습니다(MAY) — 코어 파이프라인이 재시도하거나 폴백합니다.
- `batch.units`에 존재하지 않는 유닛을 반환해서는 안 됩니다(MUST NOT).
- 중복된 `unit_id`를 반환해서는 안 됩니다(MUST NOT).
- `unit_id`를 바이트 단위로 보존해야 합니다(MUST) — 정규화 금지, 대소문자 변경 금지.
- 네트워크 재시도를 내부적으로 관찰할 수 있습니다(MAY) — `transync`는 이를 결코 보지 못합니다.
- `retry: Some(_)`를 전달하는 유닛은 재디스패치입니다: 이전 시도가 지정된 검증 계층에 의해 거부되었습니다. 구현체는 이를 사용하여 구조적 실패를 수정해야 하며(SHOULD), 무시할 수 있고(MAY), `translated_payload`에 `retry.reason` 내용을 재현해서는 안 됩니다(MUST NOT). 재시도 시 `source_payload`는 원래 디스패치와 바이트 단위로 동일합니다.

`translate_batch` 및 `extract_glossary` 둘 다의 `cancel` 인자에 대한 취소 조항:

- 토큰을 취소해서는 안 됩니다(MUST NOT). 토큰은 호출자의 것이며 실행의 모든 호출에서 공유됩니다.
- 제공자 왕복 작업을 토큰과 경쟁시키고 `TranslatorError::Cancelled`로 응답함으로써 토큰을 관찰해야 합니다(SHOULD). `tokio::select!`의 `biased;`는 필수적입니다 — 토큰을 먼저 폴링하므로 이미 취소된 실행은 요청을 결코 전송하지 않습니다. `CancellationToken::run_until_cancelled`는 반대 방향으로 편향되어 내부 퓨처를 먼저 폴링하므로 요청을 전송하게 됩니다.
- 토큰을 무시할 수 있습니다(MAY). 파이프라인은 동일한 토큰에 대해 호출을 경쟁시키고 패배한 퓨처를 삭제하며, 이는 구조상 진행 중인 `reqwest` 스타일 요청을 중단합니다. 작업이 드롭 취소 가능하지 않은 구현체(내부 `tokio::spawn`, `spawn_blocking` 클라이언트, 내부 큐)에는 이러한 안전망이 없으며 토큰 자체를 준수해야 합니다.
- 자체 취소 소스로부터 토큰이 발화하지 않았을 때 `Cancelled`를 반환할 수 있습니다(MAY). 이는 합법적이며 치명적입니다.

## HTML-세그먼트 유닛

`block_kind`가 `html`인 유닛은 `input_mode: "html_segments"`를 전달합니다. `source_payload`는 **JSON 문자열 배열을 포함하는 문자열**입니다 — 블록에서 추출된 순서화되고 엔티티 디코딩된 텍스트 세그먼트입니다. `translated_payload`는 반드시 동일한 형상, 동일한 요소 수, 동일한 순서여야 합니다(MUST). 태그, 속성, 주석 및 `script`/`style` 콘텐츠는 페이로드에 결코 나타나지 않으며 애플리케이션에 의해 다시 스플라이스됩니다. 세그먼트를 변경 없이 에코하여 보존하십시오; `partially_translated`는 html 유닛에 제공되지 않습니다; `failed_needs_fallback`은 합법적입니다.

## 관련 항목

- [사용자 지정 Translator 제공자를 구현하는 방법](../../../how-to/developer/ko/implement-a-custom-translator.md) — 작업 지향 레시피: 스캐폴딩, 오류 매핑, 연결.
- [실행 중인 번역을 취소하는 방법](../../../how-to/developer/ko/cancel-a-running-translation.md) — 호출자 측 및 구현자 측 취소 레시피.
- [번역 실행을 진단하는 방법](../../../how-to/user/ko/diagnose-a-translation-run.md) — 실패한 실행의 종료 코드, 보고서 및 stderr 줄 읽기.
- `docs/architecture/contracts.md` §1 — 이 페이지가 요약하는 완전하고 권위 있는 계약 (`CacheKey` 구성, 전체 재시도 결정 흐름(§5), 실행 취소(§5b) 포함).
