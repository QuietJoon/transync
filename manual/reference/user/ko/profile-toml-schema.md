---
type: Reference
title: Profile TOML 스키마
description: Profile TOML이 수용하는 모든 키 — 유형, 기본값, 유효 범위, CLI 플래그가 이를 덮어쓰는 방식 및 로더가 경고하는 내용.
tags: [profile, cli, glossary, batching, DCR-0026, DCR-0027]
audience: user
language: ko
sources:
  - { id: en-source, resource: manual/reference/user/en/profile-toml-schema.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:16:57Z
synced_hash: 7141e7e990a5c7c7b5edde8cb660841dfb5ce75ae424bbccfb8e8376c0730015
---
# Profile TOML 스키마

축소된 예시가 아닌 한 파일에 정리된 전체 스키마 — 모든 키를 한 번에 다룹니다.

`--profile <path>`로 전달된 프로필은 **단독으로** 로드됩니다. 함께 제공되는 기본 프로필(`crates/transync-core/profiles/default.toml`)은 그 하위 레이어가 아닙니다: 로더는 내장된 프로필 위에 커스텀 프로필을 결코 병합하지 않으므로, 파일에 없는 키는 아래 테이블의 내장 기본값으로 해소되며, 이는 항상 제공되는 프로필이 설정한 값과 일치하지는 않습니다. 제공되는 프로필과 내장 기본값이 다른 두 곳은 [`default_table_strategy`](#constraints) 및 `[batching]` 숫자들입니다.

```toml
# 필수
slug    = "default"
version = "1.0.0"

# 선택 사항
auto_glossary = false

# 필수
[system]
prompt = """
…
Translate from {{source_language}} to {{target_language}}.
…
"""

# 선택 사항
[constraints]
preserve_code_identifiers = true
preserve_urls             = true
default_table_strategy    = "row-window-first"   # 또는 "whole-block"

# 선택 사항
[render]
target_direction = "auto"                        # 또는 "ltr" / "rtl"

# 선택 사항
[batching]
target_output_tokens          = 8000
output_expansion_factor       = 2.0
target_input_tokens_per_batch = 6000
max_units_per_batch           = 8

# 선택 사항, 0개 이상
[[glossary]]
source = "agent"
target = "에이전트"
note   = "Standard term for AI agents"
scope  = "global"

[[glossary]]
source   = "cell"
target   = "감방"
scope    = "section"
sections = ["Prisons"]
```

## 최상위

| 키 | 유형 | 필수 | 기본값 | 비고 |
|---|---|---|---|---|
| `slug` | string | 예 | — | 누락 시 하드 로드 오류 발생. |
| `version` | string | 예 | — | 불투명 캐시 무효화 토큰, 결코 semver로 파싱되지 않음. `CacheKey` 축이므로 이를 변경하면 이 프로필에 키가 지정된 캐시 항목이 무효화됩니다. |
| `auto_glossary` | bool | 아니오 | `false` | 자동 추출된 후보 용어집 프리플라이트를 옵트인함. 우선순위: `--auto-glossary`/`--no-auto-glossary` 플래그 > 이 키 > 내장 `false`. |

알 수 없는 최상위 섹션 및 알려진 섹션 내부의 알 수 없는 키는 경로 자격 경고(`transync::profile` 트레이싱 타깃; `--quiet`가 아니면 CLI stderr에 한 번 출력)와 함께 무시됩니다. 향후 프로필에 새 최상위 섹션을 추가하는 것은 나누어지지 않는(non-breaking) 변경입니다; 키를 제거하거나 재용도화하는 경우 다운스트림 캐시가 무효화되도록 제공되는 기본 프로필의 `version`을 올려야 합니다.

로드 실패는 필수 필드 누락과 잘못된 형식의 TOML 단 두 가지만 존재합니다. 프로필에 대한 다른 모든 결과는 경고이며 실행은 경고가 지목하는 값으로 계속됩니다.

## `[system]`

| 키 | 유형 | 필수 | 비고 |
|---|---|---|---|
| `prompt` | string | 예 | 시스템 프롬프트 템플릿. 컴파일된 프롬프트 해시를 통해 캐시 정체성에 참여합니다. |

컴파일 시 치환되는 템플릿 변수: `{{source_language}}`, `{{target_language}}`. 다른 네임스페이스가 지정되지 않은 `{{name}}`은 경고로 보고되며 모델에 그대로 전달됩니다.

## `[constraints]`

| 키 | 유형 | 부재 시 해소 값 | 비고 |
|---|---|---|---|
| `preserve_code_identifiers` | bool | 미설정 | 명시적 프롬프트 정책 줄로 렌더링됨. 인라인 보호 레이어의 코드 스팬 정체성 검사는 이 값이 `true`일 때만 **강제**됩니다. |
| `preserve_urls` | bool | 미설정 (`true`로 작동) | 명시적 프롬프트 정책 줄로 렌더링됨. 인라인 보호 레이어의 링크/이미지 대상 정체성 검사는 이 값이 `false`가 아니면 강제됩니다. |
| `default_table_strategy` | `"whole-block"` \| `"row-window-first"` | `"whole-block"` | v0.4.0 (DCR-0026) 이후 유효함. `"row-window-first"` 하에서 추정 응답이 유효 출력 상한선을 초과하는 테이블은 팩킹 시점에 헤더를 포함하는 행 윈도우(각 윈도우는 완전한 GFM 테이블임)로 분할되고 재생성 전에 하나의 테이블로 재조합됩니다. `"whole-block"` 하에서 모든 테이블은 크기에 상관없이 하나의 유닛으로 전달됩니다; 크기 초과 테이블은 출력 예산 프리플라이트에 의해 지목된 후 제공자에서 실행을 중단합니다. 두 값 모두 무음으로 로드됩니다. 인식되지 않는 값은 로드 경고를 발생시키고 `"whole-block"`으로 해소됩니다. `--table-strategy`가 이 키를 덮어씁니다. 프롬프트로 결코 렌더링되지 않음 — 모델 지시사항이 아니라 애플리케이션 정책입니다. |

분할은 전략과 관계없이 다음 두 가지 경우에 비활성화됩니다: `[batching].target_output_tokens`가 미설정된 경우(초과할 상한선이 없음), 및 상한선에 맞는 모든 테이블, 단일 파싱 테이블이 아닌 경우, 또는 분배할 본문 행이 2개 미만인 경우. 이러한 테이블은 전체로 전송됩니다.

**부재한 키의 해소 값과 제공되는 기본값이 일치하지 않으며**, 이 불일치는 무음으로 처리됩니다. `crates/transync-core/profiles/default.toml`은 `default_table_strategy = "row-window-first"`를 설정하므로 `--profile`이 없는 실행은 크기 초과 테이블을 분할합니다. 키를 생략한 직접 작성한 프로필은 `"whole-block"`으로 해소되며 — 부재한 키가 오류가 아니기 때문에 로드 경고가 없음 — 따라서 이를 `--profile`로 전달하면 분할이 비활성화되고 크기 초과 테이블에 대한 전체 실행 제공자 중단이 복원됩니다. 프로필에 키를 작성하거나 `--table-strategy row-window-first`를 전달하는 것이 이를 다시 켜는 방법입니다.

## `[render]`

| 키 | 유형 | 기본값 | 비고 |
|---|---|---|---|
| `target_direction` | `"auto"` \| `"ltr"` \| `"rtl"` | `"auto"` | 프레젠테이션 전용: HTML 번들 이미터에 의해서만 소비됨. 시스템 프롬프트나 캐시 정체성에 결코 포함되지 않습니다. `--target-direction`이 이를 재정의합니다. `"auto"`는 대상 언어 레이블에 베스트 에포트 RTL 기본 서브태그 테이블을 적용합니다. 알 수 없는 값은 경고하고 `"auto"`로 정규화됩니다. |

## `[batching]`

| 키 | 유형 | 범위 | 미설정 시 내장 기본값 | 비고 |
|---|---|---|---|---|
| `target_output_tokens` | integer | `>= 1`, 그리고 64토큰 응답 봉투 예비량 초과 | 상한선 없음 | 제공자에게 `max_completion_tokens` (Chat Completions) / `max_output_tokens` (Responses)로 전송됨. 설정 시 출력 인지 팩킹 및 행 윈도우 테이블 분할도 구동합니다. `0` — 및 64토큰 봉투 예비량 이하의 모든 값 — 은 경고와 함께 거부되고 미설정으로 정규화됩니다; 리터럴 `0`은 결코 전송되지 않습니다. |
| `output_expansion_factor` | float | 유한함, `> 0` | `2.0` | `target_output_tokens`에 대해 배치의 크기를 조절하는 데 사용되는 추정 출력/원문 토큰 비율. `1.0` 미만의 값도 합법적입니다. `target_output_tokens`가 설정된 경우에만 효과가 있습니다. |
| `target_input_tokens_per_batch` | integer | `>= 1` | `6000` | 배치당 입력 토큰 예산. |
| `max_units_per_batch` | integer | `>= 1` | `32` | 배치당 유닛 수의 하드 캡. |

제공되는 기본 프로필은 `target_output_tokens = 8000`, `max_units_per_batch = 8` 및 `output_expansion_factor = 2.0`을 설정하고 `target_input_tokens_per_batch`를 미설정으로 남깁니다 — 따라서 `--profile`이 없는 실행은 32개가 아니라 배치당 최대 8개 유닛으로 팩킹되며 내장된 `6000` 입력 예산을 취합니다.

위의 모든 크기 조절 노브는 `0`을 동일한 방식으로 다룹니다: 경로 자격 로드 경고와 함께 거부되고, 미설정으로 정규화되며, 키는 마치 부재했던 것처럼 정확히 해소됩니다 — 결코 `1`로 최소값이 지정되지 않습니다.

`[batching].max_split_retries`는 **v0.4.0에서 제거되었습니다** (DCR-0026). 더 이상 필드가 아니며 로더의 키 허용 목록에 없고 컴파일된 프로필에 전달되지도 않습니다. 이를 여전히 포함하는 프로필은 `batching.max_split_retries`를 명시하는 일반적인 알 수 없는 키 로드 경고를 받고 그 외에는 영향받지 않습니다.

섹션들을 하나의 배치로 병합하기 위한 키는 존재하지 않습니다. `coalesce_sections`는 모든 레벨 — 프로필 키, CLI 플래그, 또는 라이브러리 옵션 — 에서 존재하지 않으며, 이를 작성하면 알 수 없는 키 경고가 발생하고 아무런 동작도 하지 않습니다.

### 이 예산들은 단일 섹션 내에서 바인딩됩니다

v0.4.0 (DCR-0027) 이후 유닛 목록은 무언가 팩킹되기 전에 모든 레벨의 모든 제목에서 분할되고 각 섹션은 스스로 팩킹됩니다. 따라서 모든 배치의 유닛은 정확히 하나의 섹션에서 나오며 어떠한 배치도 `##` / `###` 경계를 침범하지 않습니다. 위의 예산은 두 섹션을 병합할 수 없습니다: 제목이 풍부한 문서에서 실행은 예산이 아무리 크더라도 섹션당 최소 하나의 배치를 디스패치하므로 문서 크기만으로 배치 수를 더 이상 예측하지 않습니다. 이 예산들이 여전히 제어하는 것은 단일 섹션이 분할되는 방식입니다 — 예산을 초과하는 섹션만이 하나의 섹션을 여러 배치로 분할하며, 이들 모두는 여전히 해당 섹션에 국한됩니다. 제목이 없는 문서는 하나의 섹션이며 이전과 정확히 동일하게 팩킹됩니다.

배치 ID는 섹션 전반에 걸쳐 단일 문서 순서 시퀀스 `1..N`으로 유지됩니다.

규칙이 가져다주는 이점과 비용에 대해서는 [why batches stop at section boundaries](../../../explanation/user/ko/why-batches-stop-at-section-boundaries.md)를 참조하세요.

### 덮어쓰기 (Overlays)

내장 기본값과 다른 호출자 설정 `TranslateOptions` 값은 `target_input_tokens_per_batch` 및 `max_units_per_batch`에 대해 프로필 값보다 우선합니다 (내장 기본값을 전달하는 호출자는 "미설정"과 구별할 수 없음). CLI의 4개 배치 작업 플래그(`--target-output-tokens`, `--output-expansion-factor`, `--target-input-tokens-per-batch`, `--max-units-per-batch`)는 한 번의 실행 동안 이와 동일한 방식으로 프로필을 덮어쓰며, `--table-strategy`도 마찬가지입니다 — 동일한 장소에 적용되지만 `[batching]` 키가 아닌 `[constraints]` 키에 도달합니다. `--max-concurrent-batches`는 프로필 홈이 전혀 없습니다 — [CLI flags](./cli.md)를 참조하세요.

## `[[glossary]]` — 0개 이상의 항목

| 키 | 유형 | 필수 | 비고 |
|---|---|---|---|
| `source` | string | 예 | 공백이 아님. 다듬어지고 대소문자 변환되어 비교됨. 하나의 용어가 몇 번 주장될 수 있는지는 스코프에 따라 다릅니다 — [Claiming a term](#claiming-a-term)을 참조하세요. |
| `target` | string | 예 | 공백이 아님. |
| `note` | string | 아니오 | 자유 텍스트. 빈 문자열이 아닌 노트는 **렌더링되어** 에신 대시(em dash) 뒤 항목 불릿에 첨부되므로 프롬프트 텍스트로 모델에 도달합니다. |
| `scope` | `"global"` \| `"section"` | 아니오, 기본값 `"global"` | `"global"` (별칭 `"global_across_document"`)은 모든 배치의 컴파일된 프롬프트로 렌더링됩니다. `"section"` (별칭 `"conditional_on_section"`)은 `sections`가 명시하는 섹션 배치의 프롬프트로 렌더링되고 다른 배치에는 렌더링되지 않습니다; v0.4.0 (DCR-0027) 이후 유효함. |
| `sections` | array of strings | `scope = "section"`일 때 필수 비어 있지 않음; `"global"` 하에서는 의미 없음 | 섹션 범위 항목이 적용되는 위치를 명시하는 제목 텍스트 셀렉터들. |

### 섹션 셀렉터

섹션의 정체성은 해당 **제목 스택(heading stack)**입니다: 섹션을 여는 제목을 포함하여 섹션의 본문 블록을 둘러싸는 모든 제목의 일반 텍스트(가장 외부가 먼저임). 섹션 범위 항목은 다듬어지고 소문자로 변환되어 비교되어 해당 스택의 임의 제목이 항목의 셀렉터 중 임의의 것과 같을 때 해당 섹션에 적용됩니다.

- 제목 **레벨**은 결코 비교되지 않습니다. 셀렉터는 섹션이 얼마나 깊이 위치하는가가 아니라 이름에 의해 지정합니다.
- 가장 안쪽 제목이 아닌 스택과 매칭되는 것은 서브섹션 상속을 제공합니다: `sections = ["Installation"]`은 `## Installation` 아래의 `### Windows` 내부에도 적용됩니다.
- 셀렉터는 리터럴 제목 텍스트 전용입니다. 경로 표현식, 글롭 및 정규 표현식은 지원되지 않습니다.
- 전문(preamble) — 문서의 첫 제목 이전의 모든 블록 — 은 빈 제목 스택을 가지므로 어떠한 섹션 범위 항목도 적용될 수 없습니다. 글로벌 항목만 전문을 제어합니다.

### 정규화

4가지 `scope`/`sections` 조합은 말하려는 바를 의미할 수 없습니다. 각각은 경고와 함께 정규화되며; 어떠한 것도 오류가 아닙니다.

| 작성 내용 | 결과 |
|---|---|
| `scope = "global"` 항목 상의 `sections` | 해당 목록이 경고에 명시되고 삭제됩니다. 항목은 여전히 전체 문서에 적용됩니다. |
| 비어 있거나 공백으로만 구성된 셀렉터 | 해당 셀렉터가 명시되고 삭제됩니다. |
| 동일한 항목 내에서 이전 셀렉터를 반복하는 셀렉터 | 중복이 명시되고 삭제되며; 첫 표기가 살아남습니다. |
| 위의 처리 후 셀렉터 목록이 비어 있게 된 `scope = "section"` 항목 | **전체 항목**이 명시되고 삭제됩니다 — 어느 곳에도 적용되지 않기 때문입니다. |

비어 있거나 공백으로만 구성된 `source` 또는 `target`을 갖는 항목은 전송되는 대신 삭제되고 경고에 명시됩니다.

### 용어 주장

규칙은 "주장이 만날 수 있는 장소당 한 번 주장됨"이며 셀렉터 중복은 프로필 자체의 속성이므로 로드 시점에 결정됩니다:

- **하나의 용어에 대한 2개의 `global` 항목** — 프로필 순서상 첫 번째 항목이 승리하고, 나중 항목은 경고와 함께 삭제됩니다.
- **하나의 용어에 대한 2개의 `section` 항목** — 이들의 `sections` 셀렉터 세트가 분리되어 있는 동안 합법적입니다 (셀렉터는 다듬어지고 대소문자 구분 없이 비교됨). 이전 항목과 셀렉터를 공유하는 나중 항목은 동일한 첫 번째 승리 규칙 하에서 패배하고 경고와 함께 삭제됩니다.
- **하나의 용어에 대한 `global` 및 `section` 항목** — 둘 다 로드됩니다. 이것이 의도된 오버라이드 패턴이며, 어느 것이 적용되는지는 섹션별로 결정됩니다.

### 배치가 전달하는 항목

섹션의 유효 용어집은 모든 `global` 항목에 대소문자 변환된 원문 용어별로 해소된 적용 가능한 모든 `section` 항목을 더한 것입니다:

- 용어에 대한 적용 가능한 섹션 범위 주장이 없는 경우, 이를 주장하는 모든 항목이 변경 없이 통과합니다. 따라서 섹션 범위 항목이 전혀 없는 프로필은 섹션 스코프가 존재하기 전에 작성된 프로필과 바이트 단위로 동일하게 컴파일되며 동일한 캐시 키를 생성합니다.
- 적용 가능한 섹션 범위 항목은 동일한 용어의 글로벌 항목을 **무음으로** 이깁니다 — 오버라이드가 스코프의 목적이며 실수 지적 대상이 아닙니다.
- 중첩된 제목이 분리된 셀렉터 세트에서조차 생성할 수 있는 하나의 용어에 대한 여러 적용 가능한 섹션 범위 항목 중에서는 프로필 순서상 첫 번째 항목이 승리하고 각 패자는 가림(shadowing) 경고를 발생시킵니다.

섹션들이 동일한 항목 목록으로 해소되는 경우 하나의 컴파일된 프롬프트를 공유합니다. 따라서 `profile_prompt_hash` 및 `glossary_hash`는 실행당이 아니라 배치당입니다: 동일한 유효 용어집을 갖는 두 섹션은 캐시 항목을 공유할 수 있고, 속한 섹션만 다른 두 유닛은 공유를 멈춥니다.

이러한 규칙 배경의 논리 및 자동 용어집 프리플라이트와의 상호작용 방식은 [how glossary entries are resolved](../../../explanation/user/ko/how-glossary-entries-are-resolved.md)를 참조하세요.

### 진단

3가지 경고가 로드 시점이 아니라 배치가 구축되는 동안 발생하며, 모두 `transync::profile` 트레이싱 타깃(`--quiet`가 아니면 CLI stderr)으로 출력됩니다:

| 경고 | 시점 | 빈도 |
|---|---|---|
| `glossary[i] is shadowed in section "…": glossary[j] already maps "…" there and comes first in the profile …` | 2개의 적용 가능한 섹션 범위 항목이 일부 섹션에서 하나의 용어를 주장할 때 | 관찰된 첫 섹션을 명시하며 실행당 (가려진 항목, 승자) 쌍당 한 번 |
| `glossary[i] is scoped to sections […], none of which this document has; the entry reaches no prompt in this run …` | 섹션 범위 항목이 번역 중인 문서의 어떤 섹션과도 매칭되지 않았을 때 | 실행당, 항목당 한 번 |
| `glossary[i].<field> contains the control character U+XXXX; it is escaped in the rendered prompt …` | 제어 문자가 `source`, `target` 또는 `note`에 있을 때 | 실행당, 필드당 한 번 |

세 경고 중 어느 것도 항목을 삭제하지 않습니다. 두 번째는 권고 사항이며 결코 게이트가 아닙니다: 프로필은 문서 독립적이며 항목이 형제 문서를 의도한 것일 수 있습니다. 세 번째도 권고 사항입니다 — 항목은 유지되며 설명하는 이스케이프는 무조건 일어납니다.

세 가지 모두 파일의 용어집이 아니라 배치가 실제로 전달하는 용어집에 대해 발생하므로, 제어 문자 권고 사항은 자동 용어집 프리플라이트가 제공한 용어의 문자도 명시합니다. 다른 두 경고는 그럴 수 없습니다: 추출된 항목은 `scope = "global"`로 강제됩니다.

### 렌더링

배치가 전달하는 각 항목은 컴파일된 시스템 프롬프트에서 하나의 불릿으로 렌더링됩니다 — `- "source" → "target"`, 그리고 비어 있지 않은 노트가 존재할 때 ` — note` 추가. `scope` 및 `sections`는 어떠한 형태로도 결코 렌더링되지 않습니다: 스코프는 불릿을 어노테이팅함으로써가 아니라 렌더러에 전달되는 목록을 필터링함으로써 표현됩니다. 프로필을 직접 렌더링하는 것은 프로필의 **전체** 용어집을 렌더링하는데, 섹션 필터가 렌더러가 아니라 배치기에 속하기 때문입니다.

`source`, `target` 또는 `note`에 있는 백슬래시, 큰따옴표 및 제어 문자는 삭제되지 않고 렌더링 시점에 이스케이프되므로(`\n`/`\r`/`\t`, 그렇지 않으면 `\u{XXXX}`), 용어가 자신의 불릿을 종료하고 프롬프트 내부에서 자체 줄을 열 수 없습니다.

제공되는 내장 기본값은 의도적으로 **비어 있는** 활성 용어집과 함께 제공됩니다 — 하나의 용어집 항목은 대상 *언어*가 없는 대상 *형태*를 지정하므로, 제공되는 항목은 한국어로의 실행과 마찬가지로 쉽게 일본어로의 실행에도 한국어 렌더링을 적용하게 될 것이기 때문입니다.

## 관련 항목

- [How to write a Profile TOML](../../../how-to/user/ko/write-a-translation-profile.md) — 이 레퍼런스가 보완하는 작업 지향 레시피.
- [`docs/Profile_Cookbook.md`](../../../../docs/Profile_Cookbook.ko.md) — 5가지 맞춤형 레시피 (기술, 문학, 마케팅, 코드 집중, 엄격 보존). 5가지 모두 섹션 스코프 이전의 것입니다: 모든 항목이 `scope = "global"`이며 모든 레시피가 `default_table_strategy = "whole-block"`을 작성합니다.
