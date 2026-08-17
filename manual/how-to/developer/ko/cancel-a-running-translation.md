---
type: How-To Guide
title: 진행 중인 번역 취소 방법
description: `translate_with_cache`에 `CancellationToken`을 전달하여, 취소된 실행이 지불한 진행 상황을 유지하고, 자체 `Translator`가 퓨처 드롭에 의존하는 대신 토큰을 준수하도록 합니다.
tags: [providers, llm, cancellation, DCR-0024, ADR-0017]
audience: developer
language: ko
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:02Z
sources:
  - { id: en-source, resource: manual/how-to/developer/en/cancel-a-running-translation.md }
synced_hash: 20b43886d6b20eee28cbe65e3395a4dc58e7e5d92d55bb7a3ecb10bc19cfd2b3
---
# 진행 중인 번역 취소 방법

자체 코드가 `transync::translate` 또는 `transync::translate_with_cache`를 구동하고 종료 신호, 연결 해제된 클라이언트, 작업자가 철회한 작업 등 다른 이유로 실행을 중지해야 할 때 이 가이드를 사용하세요.

이것은 라이브러리 측 기능입니다. `transync` CLI는 취소 플래그를 노출하지 않습니다; CLI 실행은 프로세스를 종료할 때 중지됩니다.

## 전제 조건

- Tokio 런타임 및 `transync` 파사드에 의존하는 크레이트.
- 소유하고 있거나 구성할 수 있는 `Translator`. 직접 작성한 경우 4단계가 자신에게 적용되는 부분입니다.

## 1. 토큰 받기

`transync::CancellationToken`은 `tokio_util::sync::CancellationToken`의 재노출(re-export)입니다 — 파사드 로컬 래퍼가 아니라 실제 생태계 타입입니다. 이 재노출을 사용하면 직접 만든 `tokio-util` 의존성이 필요하지 않습니다. `tokio-util`에 직접 의존하는 경우 해당 버전은 파사드가 고정하는 버전과 통일되어야 합니다; 통일되지 않으면 침묵 속의 오동작 대신 두 크레이트를 모두 지정하는 컴파일 오류가 발생합니다.

```rust
use transync::CancellationToken;

let token = CancellationToken::new();
```

데몬에서는 새 루트 토큰을 만드는 대신 프로세스 전체 토큰에서 실행 핸들을 파생시켜 어느 신호든 작업을 중지하도록 하세요:

```rust
let token = shutdown.child_token();
```

## 2. 실행 옵션에 토큰 설정

```rust
use transync::{InMemoryCache, TranslateOptions, TransyncError};

let mut opts = TranslateOptions::default();
opts.target_language = "ko".into();
opts.cancel = Some(token.clone());
```

`TranslateOptions::cancel`은 `Option<CancellationToken>`이며 기본값은 `None`입니다. 이는 "이 실행은 취소될 수 없음"을 의미하며 필드가 존재하기 전과 정확히 동일하게 동작합니다.

**실행별로** 설정하세요. `TranslateOptions`는 `Clone`이므로 토큰을 포함하는 오래 유지되는 옵션 템플릿은 모든 작업에 동일한 토큰을 전달하게 되며, 단일 작업을 취소하면 모든 작업이 취소됩니다 — 이것이 필드가 단순 토큰이 아니라 `Option`인 이유입니다.

## 3. 소유한 캐시로 실행하고 취소된 응답 읽기

```rust
let cache = InMemoryCache::new();

match transync::translate_with_cache(&source, &opts, &translator, &cache).await {
    Ok(output) => { /* … */ }
    Err(TransyncError::Cancelled) => { /* 토큰에 의해 중지됨 */ }
    Err(e) => return Err(e),
}

// 토큰의 클론을 보유한 어느 곳에서나 — 신호 핸들러,
// 요청 핸들러, `DropGuard`:
token.cancel();
```

두 가지 사실이 해당 `match` 주변에 코드를 작성하는 방식을 결정합니다.

**취소된 실행은 `Err(TransyncError::Cancelled)`로 응답하며 부분적인 `TranslationOutput`을 결코 응답하지 않습니다.** 실행이 도달하지 못한 블록은 `fallback_source`로 표시되지 *않습니다*: 해당 마커는 "이 블록 시도가 이루어졌으나 번역할 수 없었음"을 의미하며, 이를 빌려 쓰면 정렬 맵에서 중단된 실행을 성능이 저하된 실행과 구별할 수 없게 만듭니다. 이 오류의 안정적인 코드는 `cancelled`입니다.

**지불한 진행 상황은 반환값이 아닌 캐시에 상주합니다.** 토큰이 발동되기 전에 수락된 모든 유닛은 수락될 때 `Cache`에 작성되었습니다. 따라서 사용할 진입점은 소유한 캐시를 사용하는 `translate_with_cache`입니다: 동일한 캐시에 대해 재실행하면 나머지 부분만 다시 디스패치합니다. `translate`는 호출당 일회용 메모리 내 캐시를 만들고 호출과 함께 드롭하므로 취소된 `translate`는 지불한 모든 것을 버립니다. 프로세스 전체에서 살아남는 진행 상황을 원한다면 `InMemoryCache` 대신 `transync::DiskCache::open(dir)?`를 `translate_with_cache`에 전달하세요.

토큰이 관찰되는 위치(순서대로): 실행 진입점(파싱 전 — 이미 취소된 실행은 제공자 요청을 전혀 내보내지 않음), 자동 용어집 사전 점검 직후, 각 배치의 진입점, 각 디스패치 라운드의 헤드, 모든 제공자 호출 주변, 및 모든 전송-백오프 대기 주변. 실행은 배치 팬아웃이 정착된 후 한 번 더 검사하며, **해당 검사가 권위가 있습니다**: 자매 배치가 자체 이유로 실패한 경우에도 취소된 실행은 `Cancelled`를 보고합니다.

해당 지점을 지나면 재생성, 정렬 및 렌더링만 남습니다 — 순수 CPU, I/O 없음 — 그리고 이들은 중단될 수 없습니다. 거기서 발동하는 토큰은 관찰되지 않으며 실행은 `Ok`를 반환합니다.

## 4. `Translator`에서 토큰 준수하기

두 트레이트 메서드 모두 마지막 인자로 실행의 토큰을 받습니다. 정확한 시그니처는 [`Translator` 트레이트 참조](../../../reference/developer/ko/translator-trait.md)를 참조하세요; 이 레시피는 인자로 무엇을 할 것인지에 관한 것입니다.

의무는 세 부분으로 구성됩니다:

- **토큰을 취소해서는 안 됩니다.** 토큰은 호출자의 것이며 실행의 모든 호출에 의해 공유됩니다. 관찰만 하세요.
- **왕복을 토큰과 경합시켜야(race)** 하며 토큰이 이기면 `TranslatorError::Cancelled`로 응답해야 합니다.
- **`biased;`는 핵심적인 역할을 합니다.** 토큰을 먼저 폴링하므로 이미 취소된 실행은 요청을 결코 내보내지 않습니다. `CancellationToken::run_until_cancelled`는 반대 방향으로 편향되어 있어 — 내부 퓨처를 먼저 폴링함 — 요청을 보낼 것입니다.

```rust
use async_trait::async_trait;
use transync::CancellationToken;
use transync::llm::{TranslationBatch, TranslationBatchResult, Translator, TranslatorError};

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

`extract_glossary`를 구현하는 경우 동일한 형상을 적용하세요. 번들로 제공되는 OpenAI 어댑터는 두 메서드 모두에서 정확히 이를 수행하며, 이를 통해 직접 드라이버가 해당 어댑터의 요청별 예산(기본적으로 120초)을 기다리지 않고 타입 지정된 응답을 얻을 수 있습니다.

이 패턴에서 오해하기 쉬운 두 가지 결과:

- 실행의 토큰이 발동되지 **않았을** 때 `Cancelled`를 반환하는 것은 합법적입니다 — 구현체는 자체적인 취소 소스를 보유할 수 있습니다 — 그리고 이것은 **치명적(terminal)**입니다: 파이프라인은 이를 결코 다시 디스패치하지 않습니다. 호출자에게 `Translator(Cancelled)`로 도달하며, 안정적인 코드는 엔진의 `cancelled`와 의도적으로 구별되는 `provider_cancelled`입니다.
- 용어집 사전 점검은 비대칭적입니다. `extract_glossary` *오류*는 실행을 정적 용어집으로 저하시키고 치명적이지 않고 보고됩니다; `extract_glossary` *취소*는 저하시키지 않습니다. 파이프라인은 사전 점검이 반환된 후 토큰을 다시 읽고 메서드가 무엇을 응답했든 `TransyncError::Cancelled`로 중단하므로 취소된 사전 점검은 결코 "정적 용어집으로 진행됨"으로 해결될 수 없습니다.

## 5. 토큰 무시가 허용 가능한지 결정하기

이를 준수하는 것은 SHOULD(권장)이지 MUST(필수)가 아닙니다. 왜냐하면 어느 쪽이든 실행이 끝나기 때문입니다: 파이프라인은 동일한 토큰에 대해 모든 제공자 호출을 경합시키고 패자를 **드롭(drop)**하며, `reqwest` 형상의 퓨처를 드롭하면 구성상 진행 중인 요청이 중단됩니다. 인자를 완전히 무시하더라도 실행은 여전히 `Err(TransyncError::Cancelled)`를 반환합니다.

이를 무시함으로써 포기하는 것:

- **타입 지정된 응답.** 퓨처가 `Cancelled`를 반환하는 대신 폴링 중간에 드롭되므로, `translate` / `translate_with_cache` 외부에서 직접 구동하는 모든 것은 아무것도 알지 못합니다.
- **드롭이 멈추지 않는 작업.** 내부 `tokio::spawn`, `spawn_blocking` 클라이언트, 또는 내부 큐에 이미 전달된 요청은 퓨처가 드롭된 후에도 계속 실행됩니다. 구현에 이들 중 하나라도 포함되어 있다면 토큰을 준수하는 것이 작업을 멈추는 유일한 방법이며, SHOULD는 실제로 MUST가 됩니다.
- **직접 호출 시의 신속성.** select가 없으면 취소된 호출자는 전송 스택이 적용하는 타임아웃이 만료될 때까지 기다립니다.

## 검증

- [ ] `opts.cancel`이 공유 옵션 템플릿이 아닌 실행별 토큰에서 설정되었는지 확인합니다.
- [ ] 재개를 의도하는 경우 실행이 소유한 캐시와 함께 `translate_with_cache`를 통과하는지 확인합니다; 취소된 `translate`는 아무것도 보존하지 않습니다.
- [ ] 호출자가 `TransyncError::Cancelled`를 사용자에게 보고할 실패가 아니라 "요청에 의해 중지됨"으로 취급하는지 확인합니다.
- [ ] `Translator`가 토큰을 관찰하고 토큰에 대해 `cancel()`을 결코 호출하지 않는지 확인합니다.
- [ ] select가 `biased;`이며 토큰 암(arm)이 첫 번째인지 확인합니다.
- [ ] 제공자 내부의 드롭으로 취소할 수 없는 모든 작업이 파이프라인의 퓨처 드롭이 아니라 토큰에 의해 중지되는지 확인합니다.

이 페이지가 의존하는 동작은 `crates/transync/tests/cancellation.rs`의 통합 수트에 의해 고정되어 있습니다 — 요청을 내보내지 않는 이미 취소된 실행, 중단된 전송 백오프, 부분 문서가 아닌 오류 응답, 호출자 소유 캐시에서 살아남는 진행 상황, 및 드롭되어 취소되는 토큰 무시 번역기를 포함합니다.

## 관련 정보

- 모든 메서드 시그니처 및 오류 변형 전체:
  [`Translator` 트레이트 참조](../../../reference/developer/ko/translator-trait.md).
- 제공자 직접 작성하기:
  [맞춤 Translator 제공자 구현 방법](./implement-a-custom-translator.md).
- 중지된 실행이 절반만 번역된 문서를 돌려주기를 거부하는 이유:
  [검증, 재시도 및 폴백 모델](../../../explanation/developer/ko/validation-retry-fallback-model.md).
