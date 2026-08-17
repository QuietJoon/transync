---
type: How-To Guide
title: 사용자 지정 Translator 공급자를 구현하는 방법
description: transync의 `Translator` 트레이트를 구현하는 자매 크레이트를 HEAD(취소 파라미터 포함)에 맞춰 작성하고, 실패를 오류 분류에 매핑하여 파이프라인에 전달합니다.
tags: [providers, llm, extensibility, caching, cancellation, ADR-0002, ADR-0009, DCR-0024, DCR-0029]
audience: developer
language: ko
sources:
  - { id: en-source, resource: manual/how-to/developer/en/implement-a-custom-translator.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:01Z
synced_hash: 8487153d8b0215c3ebcec5c7a913d496cc4f724fa9412cf48c7b774ca1b5c24c
---
# 사용자 지정 Translator 공급자를 구현하는 방법

transync가 아직 통신하지 않는 LLM 엔드포인트(게이트웨이, 로컬 모델 서버, 테스트용 모크 등)로 번역 배치를 전송해야 하면서 파이프라인의 나머지 부분(배치 처리, 검증, 재시도/폴백, 캐싱, 정렬 맵 배출)은 그대로 유지하고 싶을 때 이것을 사용하세요.

이 가이드는 사용자가 비동기 Rust에 익숙하고 파이프라인이 왜 그렇게 구성되어 있는지(블록 ID, 계층화된 검증, 재시도 후 폴백) 이미 알고 있다고 가정합니다. 그 배경 지식은 `docs/Developer_Guide.md` 및 `docs/architecture/contracts.md` §1을 읽어보세요 — 이 페이지는 설명이 아닌 레시피입니다.

## 먼저 참조 구현 읽어보기

이 워크스페이스의 두 공급자 크레이트가 해당 트레이트를 구현하며, 의도적으로 형태가 다릅니다:

- `crates/transync-openai` — 기본 어댑터, 두 개의 HTTP 표면(Chat Completions 및 Responses)과 이들 간의 모델 이름 디스패치.
- `crates/transync-anthropic` — 두 번째 어댑터 (DCR-0029), 하나의 HTTP 표면, **필수** 출력 상한, 그리고 공유 Structured Output 스키마를 더 좁은 방언으로 렌더링하는 스키마 프로필 패스.

그들은 코드를 공유하지 않으며 — 트레이트와 규율만 공유합니다 — 그렇기 때문에 둘 다 읽어볼 가치가 있습니다: 한 쪽에만 나타나고 다른 쪽에 없는 것은 공급자 고유의 것이고, 둘 다에 나타나는 것이 계약입니다.

**두 번째 어댑터도, 당신이 만든 어댑터도 CLI에서 도달할 수 없습니다.** `transync-cli`는 `transync-anthropic`에 대한 의존성이 없으며, `translate_cmd/provider.rs`는 정확히 두 가지 빌드 구성을 가집니다: 라이브 `TransyncOpenAI::from_env()` 경로와 `test-stub-provider` 기능 뒤의 인프로세스 스텁. 공급자 플래그도, 플러그인 조회도 없습니다. 사용자 지정 `Translator`는 이를 생성하고 `transync::translate`를 직접 호출하는 호스트 프로그램에 의해 소비됩니다 — 이는 `transync-anthropic`이 가진 상태와 동일합니다.

## 전제 조건

- `transync` 파사드 크레이트에 의존할 수 있는 Rust 크레이트(신규 또는 기존) — 여기의 워크스페이스 멤버(`crates/transync-openai` 미러링)이거나 게시된 `transync` 크레이트를 고정하는 외부 소비자로 존재.
- `async-trait` — 트레이트의 `async fn`에 필요함.
- `tokio` — 트레이트 시그니처가 취소 토큰을 전달하며 아래의 준수 패턴은 `tokio::select!`입니다.

### 취소 토큰의 유형 정보

트레이트 시그니처의 `CancellationToken`은 `tokio_util::sync::CancellationToken`입니다. 이는 파사드가 큐레이팅된 표면에 허용하는 하나의 외부 유형이며 `transync::CancellationToken`으로 도달합니다.

해당 재내보내기를 통해 이를 가져오면 자체적인 `tokio-util` 의존성이 **필요하지 않습니다**. 토큰을 구축하거나, `child_token`을 도출하거나, `DropGuard`를 보유하기 위해 직접 `tokio-util`에 의존하는 경우, 두 요구 사항이 통일되어야 하며 그렇지 않으면 토큰과 transync의 토큰이 서로 다른 유형이 되어 컴파일러가 두 크레이트의 이름을 명시하며 이를 지적합니다. 워크스페이스는 `tokio-util = { version = "0.7", default-features = false }`를 고정합니다.

## 1. 크레이트 스캐폴딩

```bash
cargo new --lib transync-myprovider
```

파사드 크레이트에만 의존하세요 — `transync-core`나 `transync-syntax`에 직접 의존하지 마세요; 이들은 semver 방화벽 외부의 엔진 내부입니다 (`docs/architecture/contracts.md` §0, 티어 (c)). 두 트리 내 어댑터 모두 동일한 의존성 엣지 목록을 선언하며 이는 합리적인 출발점입니다:

```toml
[dependencies]
transync    = { path = "../transync" }   # 또는 crates.io 버전 고정
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
thiserror   = "1"
async-trait = "0.1"
tokio       = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
# 공급자의 전송에 필요한 것 추가 (reqwest, secrecy, url 등)
```

크레이트를 트리 내 워크스페이스 멤버로 추가하는 경우 워크스페이스 루트 `Cargo.toml` 파일의 `members`에도 해당 경로를 추가하세요.

## 2. 트레이트 구현

`crates/transync-core/src/llm.rs`에 대해 시그니처를 검증하세요 — 이를 설명하는 줄글이 아니라 그것이 기저의 진실입니다. `translate_batch`가 유일한 필수 메서드입니다; `fingerprint`, `tokenizer_hint` 및 `extract_glossary`는 기본값이 지정되어 있습니다.

두 비동기 메서드 모두 후행 `cancel: &CancellationToken`을 받습니다. 이는 HEAD에 대해 컴파일됩니다:

```rust
use async_trait::async_trait;
use transync::CancellationToken;
use transync::llm::{
    OutputKind, TranslationBatch, TranslationBatchResult, Translator, TranslatorError,
    UnitResult,
};

pub struct MyProvider {
    // api key, model id, http client, …
}

impl MyProvider {
    /// 공급자 왕복(round-trip) 1회. 트레이트 메서드가
    /// 아래의 경합 전용이 되도록 하고, 이 퓨처가 경합에서 졌을 때
    /// 드롭되는 대상이 되도록 트레이트에서 분리해 둡니다.
    async fn round_trip(
        &self,
        batch: &TranslationBatch,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        // 1. `batch.units`를 공급자의 요청 형태로 렌더링합니다.
        //    각 유닛의 `block_kind` + `input_mode`가 구조적 형태를 알려주며,
        //    `constraints`는 보존해야 할 불변성이고, `context`는 선택적이지만 품질을 향상시킵니다.
        // 2. 원하는 전송 방식을 통해 요청을 전송합니다 — transync는
        //    내부적으로 수행하는 네트워크 재시도를 결코 보지 못합니다.
        // 3. 응답을 요청된 유닛당 하나의 `UnitResult`로 파싱합니다.
        // 4. 동일한 `batch_id`로 `TranslationBatchResult`를 반환합니다.
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id.clone(),
            detected_source_language: None,
            units: batch
                .units
                .iter()
                .map(|u| UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Translated,
                    translated_payload: String::new(), // 모델의 번역 결과
                    warnings: vec![],
                })
                .collect(),
        })
    }
}

#[async_trait]
impl Translator for MyProvider {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(TranslatorError::Cancelled),
            r = self.round_trip(&batch) => r,
        }
    }
}
```

`biased;`는 장식이 아닌 하중을 견디는 장치입니다: 토큰을 먼저 폴링하므로 이미 취소된 실행은 결코 요청을 발송하지 않습니다. `CancellationToken::run_until_cancelled`는 다른 방향으로 편향되어 있어 내부 퓨처를 먼저 폴링하므로 요청을 발송하게 됩니다. 두 트리 내 어댑터 모두 두 비동기 메서드에서 정확히 이 형태를 사용합니다.

전달받은 토큰을 결코 직접 취소하지 마세요. 이는 호출자의 것이며 실행의 모든 호출에 걸쳐 공유됩니다. 취소 의무의 전체 집합과 퓨처 드롭이 이미 취소했을 때에도 토큰을 준수해야 하는 이유는 [실행 중인 번역을 취소하는 방법](./cancel-a-running-translation.md)에 나와 있습니다.

`block_kind`가 `html`인 유닛은 다른 처리가 필요합니다: `source_payload`가 미가공 텍스트가 아니라 텍스트 세그먼트의 JSON 배열을 담고 있는 문자열입니다 — 해당 분기를 작성하기전에 [`Translator` 트레이트 참조](../../../reference/developer/ko/translator-trait.md) 및 `contracts.md` §1 ("HTML-segment units")을 참조하세요. `input_mode`가 `table_row_window`인 유닛은 새로운 코드가 필요하지 **않습니다**: 해당 페이로드는 완전한 GFM 테이블이며 전체 테이블과 정확히 동일하게 번역됩니다.

## 3. 공급자 실패를 `TranslatorError`에 매핑하기

`TranslatorError`는 `#[non_exhaustive]`이며 HEAD에서 **13개**의 변형을 가집니다 — `Network`, `Authentication`, `RateLimited { retry_after }`, `MalformedResponse`, `Unsupported`, `ContentFiltered`, `OutputCeilingExhausted`, `ContextWindowExceeded`, `ModelRefused`, `ResponseTooLarge`, `ProviderRejected { status, message }`, `Cancelled`, `Other`. 와일드카드 아암으로 일치시키세요; 각 변형의 안정한 코드가 포함된 전체 테이블은 [참조](../../../reference/developer/ko/translator-trait.md)에 있습니다.

어느 변형을 선택할지는 두 가지 규칙이 지배합니다:

1. **파이프라인은 `Network` 및 `RateLimited`만 재시도합니다** (`transync-core::pipeline::dispatch::translate_with_provider_retries`). 다른 모든 변형은 터미널이며 일단 마주치면 실행을 중단합니다 (ADR-0017; 배치별 폴백 단계는 없으며 검증 분기를 통한 유닛별 폴백만 있음).
2. **당신이 긋는 선은 "축어적 재전송이 다르게 돌아올 가능성이 있는가"입니다** — 심각도가 아닙니다. 소진된 출력 상한은 상한이 요청에 포함되어 전달되었으므로 터미널입니다. 공급자 측 `failed` 상태는 요청이 수락되었고 공급자가 자체 측에서 포기한 것이므로 일시적(transient)입니다.

두 어댑터 모두 크레이트 프라이빗 `ProviderError`를 유지하고, 신호를 읽는 지점에서 분류하며, 경계 함수가 이름만 바꾸도록 합니다. 다음은 11개 아암 모두가 포함된 `transync-openai/src/error.rs`의 맵입니다:

```rust
match err {
    ProviderError::Transport(s) => TranslatorError::Network(s),
    ProviderError::Auth(s) => TranslatorError::Authentication(s),
    ProviderError::RateLimited => TranslatorError::RateLimited { retry_after: None },
    ProviderError::RateLimitedAfter(retry_after) => {
        TranslatorError::RateLimited { retry_after }
    }
    ProviderError::Malformed(s) => TranslatorError::MalformedResponse(s),
    ProviderError::ContentFiltered(s) => TranslatorError::ContentFiltered(s),
    ProviderError::OutputCeilingExhausted(s) => TranslatorError::OutputCeilingExhausted(s),
    ProviderError::ModelRefused(s) => TranslatorError::ModelRefused(s),
    ProviderError::ResponseTooLarge(s) => TranslatorError::ResponseTooLarge(s),
    ProviderError::Rejected { status, message } => {
        TranslatorError::ProviderRejected { status, message }
    }
    ProviderError::Other(s) => TranslatorError::Other(s),
}
```

`transync-anthropic`의 맵은 서로 다른 아암 세트에 대한 동일한 아이디어입니다: 여기에는 단독 `RateLimited`가 없고(여기서 비율 제한된 응답은 헤더가 파싱된 결과를 부재를 포함하여 항상 지님), `ContextWindowExceeded → ContextWindowExceeded`를 추가합니다. OpenAI는 이에 매핑되는 신호가 없는 반면 해당 공급자는 *이를 명시적으로* 말하기 때문입니다. 당신의 고유한 프라이빗 열거형에 어떤 변형이 필요한지는 두 목록을 복사하는 것이 아니라 공급자가 실제로 구별하는 바에 따라 결정됩니다.

### HTTP 상태 → 변형

`crates/transync-openai/src/client/classify.rs`가 이에 대한 예시입니다. 어느 4xx 상태가 재시도할 가치가 있는지 재발명하기보다는 테이블의 *형태*를 재사용하세요:

| 상태 | 공급자 오류 | 재시도 가능 여부 |
|---|---|---|
| 401, 403 | `Auth` (순수 본문 발췌, `HTTP <status>:` 접두사 없음) | 아니오 |
| 429 | `RateLimitedAfter(파싱된 Retry-After)` | 예 |
| 408, 409, 425 | `Transport` — 흔히 재시도 가능하며 코드만으로는 명확하지 않음 | 예 |
| 기타 모든 400–499 | `Rejected { status: Some(code), message }` | 아니오 |
| 기타 모든 것 (5xx 이상) | `Transport` | 예 |

**기타 4xx를 `Other`로 축소하지 마세요.** 이들은 상태를 타입화된 `u16`으로 전달하며 그것이 핵심 포인트입니다: `Some(404)`는 존재하지 않는 모델 이름 — 작업자 결함 — 으로 소비자가 글을 파싱하지 않고도 400과 구별할 수 있어야 합니다. 미분류 포괄 항목으로 전송하는 것은 `ProviderRejected.status`가 제거하기 위해 도입된 결함입니다. `Other`는 다운스트림에서 *알 수 없음*으로 읽히고 알 수 없는 것에 대한 정직한 처리는 재시도이기 때문입니다. 공급자가 제공할 HTTP 상태가 없는 경우에도 `ProviderRejected`가 여전히 적합합니다 — `status`가 `Option<u16>`이므로 `None`을 표현할 수 있습니다.

`transync-anthropic`은 500–599 대역 밖에 위치하는 `overloaded_error`인 529를 일시적 세트에 명시적으로 추가합니다.

상태가 확인되기 전에 발생한 실패는 두 어댑터에서 동일한 방식으로 분류됩니다: `reqwest` 타임아웃 및 연결 실패는 `Transport`가 되고, 디코드 실패는 `Malformed`가 되며, 다른 모든 것은 `Transport`가 됩니다. 응답 크기 상한을 초과하는 본문은 상한을 명시하는 터미널 `ResponseTooLarge`가 됩니다.

### 엔벨로프 신호 → 변형

200 응답이 성공을 의미하지는 않습니다. 메시지 본문을 읽기 **전에** 정지/종료 신호를 읽고 매핑하세요:

| 공급자 신호 | 변형 |
|---|---|
| Chat `finish_reason: "length"` | `OutputCeilingExhausted` |
| Chat `finish_reason: "content_filter"` | `ContentFiltered` |
| Chat `refusal` 필드 (`finish_reason: "stop"` 하에서 도착함) | `ModelRefused` |
| Responses `incomplete` + `incomplete_details.reason: "max_output_tokens"` | `OutputCeilingExhausted` |
| Responses `incomplete` + `reason: "content_filter"` | `ContentFiltered` |
| Responses `incomplete` + 기타 또는 부재하는 이유 | `Other` (의도적으로 빌려온 이름이 아님) |
| Responses `failed`, `cancelled`, `queued`, `in_progress` | `Transport` (재시도 가능 — 5xx에 해당하는 엔벨로프 수준의 아날로그) |
| Messages `stop_reason: "max_tokens"` | `OutputCeilingExhausted` |
| Messages `stop_reason: "refusal"` **와 함께** `stop_details.category` 지님 | `ContentFiltered` |
| Messages `stop_reason: "refusal"` 카테고리 없음 | `ModelRefused` |
| Messages `stop_reason: "model_context_window_exceeded"` | `ContextWindowExceeded` |

거절은 옆의 어떤 텍스트보다 **우선합니다**. 전체 엔벨로프를 순회하고 거절 세그먼트를 수집하여 비어 있지 않은 출력 텍스트가 도착한 경우에도 `ModelRefused`로 실패 처리하세요 — 그렇지 않으면 모델의 거절 문구를 마치 문서인 것처럼 번역하게 됩니다.

공급자가 제어하는 모든 문자열이 크레이트를 벗어나기 전에 캡을 적용하세요. 두 어댑터 모두 거절 텍스트, 상태 어휘, `incomplete_details` 이유 및 오류 본문을 `… (N bytes total, truncated)` 접미사가 붙은 512바이트 문자기반 안전 발췌문으로 자르므로 적대적이거나 오작동하는 게이트웨이가 에코된 문서로 호스트의 stderr 및 로그를 범람시킬 수 없습니다. 발췌문이 아닌 **미가공** 신호에 대해 분류하세요 — 진단 세부 사항이 변형을 결정해서는 안 됩니다.

## 4. 인스턴스가 상호 교환 가능하지 않은 경우 `fingerprint()` 재정의

기본값은 유형 이름에서 핑거프린트를 도출하며 이는 `MyProvider`의 모든 인스턴스가 동일한 입력에 대해 동일한 출력을 생성할 때만 올바릅니다. 구조체를 설정할 수 있는 경우(모델, 엔드포인트, API 표면, 추론 노력 등), 출력을 변경할 수 있는 모든 축을 커버하도록 이를 재정의하세요 — 그렇지 않으면 공유된 `Cache`가 하나의 설정에 대한 번역을 다른 설정에 대해 재플레이할 수 있습니다:

```rust
fn fingerprint(&self) -> ProviderFingerprint {
    let base = self.base_url.as_ref().map(Url::as_str).unwrap_or(DEFAULT_BASE_URL);
    let effort = self.effort.map(Effort::as_str).unwrap_or("default");
    ProviderFingerprint::new("myprovider", &[&self.model.0, base, effort])
}
```

과도한 구별(실제로 출력을 변경하지 않는 추가 축)은 발생할 수 있었던 캐시 히트 시 중복 재번역 비용만 초과로 유발합니다. 불충분한 구별은 공유 캐시가 한 설정의 출력을 다른 설정의 슬롯으로 누출시키게 만듭니다 — 피해야 할 방향입니다.

**모든 인스턴스별 축이 핑거프린트에 속하는 것은 아닙니다.** 두 어댑터 모두 `with_timeout(Duration)` 빌더를 지니며 둘 다 의도적으로 이를 제외합니다: 타임아웃은 응답이 제시간에 도착하는지 *여부*만 변경할 뿐 공급자가 반환하는 *내용*은 결코 변경하지 않으므로, 이를 포함하면 단순히 인내심을 다르게 조절하는 두 호스트 간에 캐시 네임스페이스가 분할됩니다. `transync-anthropic`은 다른 이유로 한 가지를 더 제외합니다 — 이것의 `anthropic-version` 헤더는 인스턴스별 값이 아니라 크레이트 전역 상수이므로 동일한 빌드의 두 인스턴스가 이에 대해 불일치할 수 없습니다. 자체 공급자가 어떤 형태로든 노브를 키우는 경우 이를 제외하고 문서 주석에 명시하세요; 나중에 핑거프린트를 감사하는 독자는 의도적인 누락과 놓친 축을 구별할 다른 방법이 없습니다.

또한 `tokenizer_hint()`를 고려하세요: 모델 이름이 OpenAI 형태가 아닌 경우 엔진의 이름 휴리스틱이 잘못 추측하도록 두지 말고 재정의하세요. 배치 예산 추정은 소프트 캡이므로 근사치여도 괜찮습니다 — *선언되지 않은* 근사치는 괜찮지 않습니다. `transync-anthropic`은 모든 모델에 대해 고정된 `Some(TokenizerHint::Cl100kBase)`를 반환하며 이것이 명시된 근사치의 모습입니다.

`extract_glossary()`는 기본적으로 `Ok(None)`(미지원)으로 유지됩니다; 공급자가 한 번의 사전 점검 호출에서 후보 용어를 유용하게 수확할 수 있는 경우에만 구현하세요. 구현하는 경우 이 역시 `cancel`을 받으며 `Ok(Some(vec![]))`("지원하지만 아무것도 찾지 못함")가 `Ok(None)`과는 다른 답임을 기억하세요.

## 5. 동작 계약 충족하기

파이프라인은 모든 `translate_batch` 호출에 대해 이것들이 성립한다고 가정합니다(`contracts.md` §1의 "Behavior contract"); 위반 시 출력이 손상되거나 파이프라인이 선의로 할당한 재시도/폴백 슬롯을 낭비하게 됩니다:

- [ ] `batch.units`의 유닛당 정확히 하나의 `UnitResult` — 중복 없음, 추가 없음, 누락 없음.
- [ ] 각 결과의 `unit_id`는 해당 요청 유닛의 `unit_id`와 바이트 단위로 일치함 — 정규화 없음, 대소문자 변경 없음.
- [ ] 번역할 수 없는 유닛은 `Translated`로 위장된 가짜 또는 부분 번역이 아니라 `output_kind: FailedNeedsFallback`을 받음.
- [ ] `translated_payload`에 `RetryContext.reason` 문자열이 결코 누출되지 않음 — 해당 필드는 페이로드에 *대한* 사이드 채널이지 페이로드 *내부*의 콘텐츠가 아닙니다.
- [ ] `retry` 필드가 `Some(_)`인 유닛은 재발송입니다: 해당 `source_payload`는 원래 시도와 바이트 단위로 동일하며, 명시된 구조적 실패를 수정하기 위해 `retry.rejected_by` / `retry.reason`을 사용해야 함(SHOULD)(무시할 수도 있지만(MAY) 출크에 `reason`을 에코해서는 안 됨(MUST NOT)).
- [ ] 오류는 파이프라인의 재시도 정책이 예상하는 변형에 매핑됨(위의 §3) — 올바른 `Network`/`RateLimited` 분류라면 살아남았을 실행을 터미널로 잘못 매핑하여 중단시키거나, 일시적으로 잘못 매핑하여 예산을 태운 후 어차피 중단시킴.

그리고 두 비동기 메서드의 `cancel` 인자에 대해:

- [ ] 토큰을 수락하고 관찰하며 결코 이를 취소하지 않음.
- [ ] 경합은 `biased;`이므로 이미 취소된 실행은 요청을 발송하지 않음.
- [ ] 작업이 드롭 취소 가능하지 **않은** 경우(내부 `tokio::spawn`, `spawn_blocking` 클라이언트, 내부 큐) 토큰을 직접 준수함. 파이프라인의 유일한 폴백은 퓨처를 드롭하는 것이며 이는 해당 항목에 도달하지 않음.

토큰이 발화하지 *않았을* 때 `TranslatorError::Cancelled`를 반환하는 것은 합법적이며 — 자체적인 취소 소스를 보유할 수 있음 — 이는 터미널이고 결코 재발송되지 않습니다.

## 6. 연결하기

등록 단계 없음 — `transync::translate`는 제네릭 바운드(`T: Translator + ?Sized`이므로 `&dyn Translator`도 작동함)를 통해 모든 `Translator`를 받으며, 코어 라이브러리는 공급자 크레이트에 대한 컴파일 시점 지식이 없습니다:

```rust
let translator = MyProvider::new(/* … */);
let mut opts = transync::TranslateOptions::default();
opts.target_language = "ko".into();
let output = transync::translate(&source, &opts, &translator).await?;
```

`TranslateOptions`는 `#[non_exhaustive]`이며, 이것이 구조체 리터럴 대신 기본값 후 할당 형태를 취하는 이유입니다.

실행을 취소 가능하게 만들려면 옵션에 토큰을 넣으세요 — `opts.cancel = Some(token)`; 기본값은 `None`으로 실행이 취소될 수 없으며 필드가 존재하기 전과 정확히 동일하게 동작합니다. 이를 발판 삼아 구축하기 전에 알아둘 만한 두 가지 결과:

- 취소된 실행은 `Err(TransyncError::Cancelled)`를 반환하며 결코 부분적인 `TranslationOutput`을 반환하지 **않습니다**. 도달하지 않은 블록은 `fallback_source`로 표시되지 않습니다 — 해당 마커는 "시도했으나 번역할 수 없음"을 의미하며 이는 다른 사실입니다.
- 비용을 지불한 진행 상황은 반환값이 아니라 **캐시**에 남습니다. 직접 소유한 캐시와 함께 `transync::translate_with_cache(&source, &opts, &translator, &cache)`로 교체하면 취소된 실행은 재시도 시 비행 중인 배치의 한 라운드 비용만 들게 됩니다; 일반 `translate`는 호출당 일회용 캐시를 구축하고 아무것도 유지하지 않습니다.

이 두 가지 측면에 대한 레시피는 [실행 중인 번역을 취소하는 방법](./cancel-a-running-translation.md)에 나와 있습니다.

## 참조 구현 읽기

`crates/transync-openai/src/client.rs` 및 `crates/transync-anthropic/src/client.rs`는 둘 다 *흐름 전용*입니다 — 공급자별로 다른 부분에 대해서는 하위 모듈과 함께 읽으세요:

| 파일 | 소유하는 사항 |
|---|---|
| `openai/client/dispatch.rs` | 모델 이름 → 어느 HTTP 표면인지 |
| `openai/client/chat.rs` | Chat Completions DTO, 본문 빌더, 엔벨로프 리더 |
| `openai/client/responses.rs` | Responses API DTO, 본문 빌더, 엔벨로프 리더 |
| `openai/client/endpoint.rs` | 엔드포인트 경로, base-URL 정규화, 경로 결합 |
| `openai/client/transport.rs` | 응답 크기 상한이 있는 베어러 인증 POST |
| `openai/client/classify.rs` | HTTP 상태 / `reqwest` 실패 → 공급자 오류 (§3의 테이블) |
| `anthropic/client/messages.rs` | Messages DTO, 두 본문 빌더 모두, 단일 엔벨로프 리더 |
| `anthropic/client/schema.rs` | 공유 스키마 객체를 더 좁은 방언으로 렌더링 |
| `anthropic/client/transport.rs` | 본문 상한이 있는 `x-api-key` POST |
| `anthropic/client/classify.rs` | 동일한 상태 테이블 및 이름 명시 529 |

두 `client.rs` 파일에서 복사할 가치가 있음을 보여주는 요소: 엔드포인트를 해석하고, POST를 수행하며, 엔벨로프 리더에게 미가공 바이트를 전달하는 하나의 공유 `round_trip()` 헬퍼 — 전송 및 잘못된 형식의 엔벨로프 처리가 크레이트가 하나의 HTTP 표면을 지원하든 둘을 지원하든 정확히 한 번만 존재합니다.

그들의 생성자도 복사할 가치가 있습니다. 두 크레이트 모두 검증되지 않은 `new`(아무것도 검증하지 않음), 검증된 `try_new`, 그리고 변수를 읽은 후 `try_new`가 되는 `from_env`를 제공하므로 — 환경이 결코 약한 문이 되지 않습니다. 공백이 추가된 모델 id는 저장된 문자열이 와이어 값이자 `fingerprint()` 축이기 때문에 트림되지 않고 거부됩니다.

## 이미 준비된 사항과 준비되지 않은 한 가지

워크스페이스는 `0.3.0`에 있으며, `contracts.md`가 v0.4.0 파괴적 변경이라 부르는 파괴적 변경 창이 `CHANGELOG.md`의 `## [Unreleased]` 아래 준비되어 있습니다. 오늘 새로 시작하면 이것들은 단순히 계약의 형태이며 마이그레이션 비용이 들지 않습니다:

- `fingerprint()`, `tokenizer_hint()` 및 `extract_glossary()`는 모두 **기본값**이 지정된 메서드입니다. 4단계가 적용되는 경우에만 재정의를 작성합니다.
- `Cache` 트레이트는 **7개**의 메서드를 가지며, 아마도 어느 것도 직접 구현하지 않을 것입니다: `get` / `put` / `evict`는 필수이며 이미 오류를 반환할 수 있고(`Result<_, CacheError>`), `get_document_meta` / `put_document_meta` / `get_glossary_extraction` / `put_glossary_extraction`은 `Ok(None)` / `Ok(())`로 기본 설정되어 있습니다. 두 개의 백엔드가 제공되고 파사드에서 재내보내집니다 — `InMemoryCache` 및 디스크 기반 `DiskCache` — 따라서 공급자 크레이트는 일반적으로 트레이트를 구현하기보다 소비합니다.
- `TranslationUnit`은 `retry` 필드가 이미 존재하는 `#[non_exhaustive]`입니다; 구조체 리터럴이 아닌 `TranslationUnit::new(unit_id, block_kind, input_mode, source_payload, source_hash)` 및 `with_*` 빌더를 통해 생성하세요 (예: 테스트에서).
- 모든 `TranslatorError` 변형은 `stable_code()`를 이미 지니고 있습니다. 올바른 변형을 선택하면 다운스트림 소비자가 키를 지정하는 와이어 코드가 무료로 정답이 됩니다.

무료가 **아닌** 한 가지: `translate_batch` 및 `extract_glossary`는 미출시 v0.4 창에서 `cancel: &CancellationToken` 파라미터를 얻었습니다 (DCR-0024). 이는 필수 메서드에 대한 시그니처 변경이므로 어떤 기본값도 흡수할 수 없습니다 — 0.3.0에 맞춰 작성된 구현은 파라미터가 추가될 때까지 컴파일되지 않습니다.

## 참조: 전체 트레이트 및 오류 세트

엄격하고 완전한 목록 — 모든 메서드 시그니처, 안정한 코드가 포함된 모든 오류 변형, 필드별 동작 계약 — 을 보려면 이 레시피에서 다시 도출하기보다 [ `Translator` 트레이트 참조](../../../reference/developer/ko/translator-trait.md)를 참조하세요. 강등이 아닌 터미널이 파이프라인의 답인 이유는 [계층화된 검증 및 제한된 재시도/폴백인 이유](../../../explanation/developer/ko/validation-retry-fallback-model.md)에 있습니다. 매핑이 CLI에 도달할 때 작업자가 보게 되는 내용은 [번역 실행을 진단하는 방법](../../user/ko/diagnose-a-translation-run.md)에 나와 있습니다.

## 검증

```bash
cargo build -p transync-myprovider
cargo test -p transync-myprovider
```

그런 다음 작은 샘플 문서에 대해 `transync::translate`를 통해 실제 배치를 실행하고 `output.validation_summary`가 예상 유닛을 `fallback_source`가 아닌 `translated`로 보여주는지 확인하세요 — 기술적으로 컴파일되지만 §5의 동작 계약을 위반하는 공급자는 일반적으로 설명되지 않은 폴백이나 파이프라인 중단 `Err`로 가장 먼저 거기에 나타납니다.
