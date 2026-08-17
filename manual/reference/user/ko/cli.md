---
type: Reference
title: CLI 레퍼런스
description: "`transync translate` 및 `transync serve`에 대한 모든 플래그, 기본값, 제한, 종료 코드, 환경 변수 및 stderr 라인 설명."
tags: [cli, reference, flags, exit-codes, serve, SCN-12, SCN-13, DCR-0026, DCR-0028]
audience: user
language: ko
sources:
  - { id: en-source, resource: manual/reference/user/en/cli.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:16:57Z
synced_hash: e34d158712425a957780688d456435769c1a3fb96eb1ada912750cbffc1c92b9
---
# CLI 레퍼런스

`transync` 바이너리에는 두 가지 하위 명령이 있습니다: Markdown 문서를 번역하고 출력 세트를 게시하는 `transync translate`와 HTTP를 통해 루프백에서 렌더링된 번들 디렉터리를 서빙하는 `transync serve`. 두 명령은 종료 코드 공간과 stderr 스트림을 공유할 뿐 다른 것은 공유하지 않습니다: `serve`에는 제공자, 프로필 및 상세 플래그(verbosity flags)가 없습니다.

`--help` 및 `--version`은 인자 파서에 의해 처리되며 `0`으로 종료됩니다.

## `transync translate`

| 플래그 | 필수 여부 | 기본값 | 의미 |
|---|---|---|---|
| `--input <path>` | 예 | — | 원문 Markdown 파일. |
| `--max-input-bytes <n>` | 아니오 | `67108864` (64 MiB) | 파싱하기 전 더 큰 `--input`을 거부합니다 (종료 `2`). 파일 메타데이터를 신뢰하는 대신 `n + 1` 바이트를 읽습니다. 고정 4 MiB로 제한되는 `--profile` / `--system-prompt-file`에는 적용되지 않습니다. |
| `--output <path>` | `--map`과 함께 필수 (`--out-dir` 사용 시 제외) | — | 번역된 Markdown이 저장될 경로. |
| `--map <path>` | `--output`과 함께 필수 (`--out-dir` 사용 시 제외) | — | 정렬 맵 JSON이 저장될 경로. |
| `--out-dir <dir>` | `--output` / `--map` / `--html-out`과 상호 배타적 | — | 전체 출력 세트를 하나의 디렉터리에 게시합니다. [출력 대상](#output-destinations)을 참조하십시오. |
| `--html-out <dir>` | 아니오 | — | 이 디렉터리에 6개 파일로 구성된 탐색 가능한 데모 번들을 작성합니다. |
| `--strict-csp` | 아니오 | off | 내보낸 번들의 `index.html`에 `Content-Security-Policy` `<meta>`를 날인합니다. [`--strict-csp` 정책](#--strict-csp-policy)을 참조하십시오. |
| `--title <text>` | 아니오 | 원문 문서의 첫 H1, 없으면 `transync` | 번들 `<title>`. 한 번 트림됩니다; 빈 값 또는 공백만 있는 값은 속행되지 않고 종료 `1`이 됩니다. 번들 전용 — `out.md`, 정렬 맵 또는 제공자에 도달하지 않습니다. |
| `--validation-report <path>` | 아니오 | — | 유닛별 검증 보고서를 자체 JSON 파일로 작성합니다. 항상 `validation-report.json`을 작성하는 `--out-dir` 아래에서는 효과가 없습니다. |
| `--target-language <label>` | 예 | — | 불투명한(opaque) 비어 있지 않은 라벨. BCP-47 태그로 검증되지 않습니다. |
| `--source-language <label>\|auto` | 아니오 | `auto` | `auto`는 모델 측 감지를 위해 예약된 유일한 센티널입니다: 대소문자를 구분하지 않고 인식되며 정준 소문자로 저장됩니다. 다른 모든 값은 대소문자가 보존되는 불투명한 라벨입니다. |
| `--target-direction <rtl\|ltr\|auto>` | 아니오 | 프로필 `[render].target_direction`, 없으면 `auto` | 프레젠테이션 전용: 번들의 대상 패널 텍스트 방향. `ltr`은 `dir` 속성을 전혀 내보내지 않습니다. `out.md`나 정렬 맵에 결코 영향을 미치지 않습니다. |
| `--profile <path>` | 아니오 | 내장된 기본 프로필 | 사용자 지정 프로필 TOML. 내장된 프로필을 **대체**하며; 병합되지 않습니다. [프로필 TOML 스키마](./profile-toml-schema.md)를 참조하십시오. |
| `--system-prompt <text>` | `--system-prompt-file`과 상호 배타적 | — | 활성 프로필의 `[system].prompt` 본문을 대체합니다. 템플릿 변수는 여전히 치환됩니다. |
| `--system-prompt-file <path>` | `--system-prompt`와 상호 배타적 | — | 파일에서 읽어오는 동일한 기능 (4 MiB 제한). |
| `--model <id>` | 아니오 | `$TRANSYNC_OPENAI_MODEL`, 없으면 `gpt-5-chat-latest` | 제공자에게 전달되는 모델 ID. 환경 변수가 먼저 해소되므로 파서 기본값이 없습니다. 명시적으로 빈 값은 종료 `1`입니다. |
| `--base-url <url>` | 아니오 | `$TRANSYNC_OPENAI_BASE_URL`, 없으면 `https://api.openai.com` | 제공자 엔드포인트. 동일한 이유로 파서 기본값이 없습니다. 파싱할 수 없는 URL은 종료 `1`입니다. |
| `--cache-dir <path>` | 아니오 | 없음 — 실행당 별도의 메모리 내 캐시 | 이 디렉터리에 디스크 기반 번역 캐시를 열고(없으면 생성) `transync-cache.jsonl`에 기록합니다. 동일한 프로필, 모델 및 언어 아래에서 동일한 문서에 대한 이후 실행은 변경된 내용만 다시 디스패치하며, 완전 캐시 히트 `--source-language auto` 실행은 첫 번째 실행의 감지된 언어를 여전히 보고합니다. 한 번에 하나의 작성자만 허용됩니다. 열 수 없는 디렉터리는 한 번 경고하고 실행은 새 메모리 내 캐시에서 속행됩니다. |
| `--force` | 아니오 | off | `--html-out` 디렉터리에 쓰거나 transync가 작성하지 않은 파일을 보유한 `--out-dir` 대상을 대체할 때 외부 파일 거부를 무효화합니다. transync 자체의 이전 번들이나 출력 세트를 오버라이드하는 데는 필요하지 않습니다. |
| `--target-output-tokens <n>` | 아니오 | `8000` (내장된 기본 프로필 제공) | 배치당 출력 토큰 상한: 제공자에게 응답 상한으로 전송되며 패커가 배치하는 예산으로 사용됩니다. `[batching].target_output_tokens`를 덮어씁니다. `0`은 상한을 비활성화합니다. [배치 처리 및 동시성](#batching-and-concurrency)을 참조하십시오. |
| `--output-expansion-factor <f>` | 아니오 | `2.0` | 배치를 배치하는 데 사용되는 추정 출력 대 원문 토큰 비율. 0보다 큰 유한한 숫자여야 합니다; `1.0` 미만의 값도 합법적입니다. |
| `--target-input-tokens-per-batch <n>` | 아니오 | `6000` | 소프트 배치당 입력 토큰 예산; `n >= 1`. |
| `--max-units-per-batch <n>` | 아니오 | `8` (내장된 기본 프로필 제공) | 배치당 유닛 수에 대한 하드 캡; `n >= 1`. |
| `--table-strategy <whole-block\|row-window-first>` | 아니오 | `row-window-first` (내장된 기본 프로필 제공) | 추정 응답이 출력 상한을 초과하는 테이블에 일어나는 일. `row-window-first`는 패킹 시점에 헤더를 전달하는 행 윈도우로 분할하고 나중에 하나의 테이블로 재조합합니다; `whole-block`은 전체를 전송합니다. `[constraints].default_table_strategy`를 덮어씁니다. 다른 모든 값은 종료 `1`입니다. 상한이 없을 때는 비활성입니다. |
| `--max-concurrent-batches <n>` | 아니오 | `6` | 인플라이트(in-flight) 제공자 요청; `n >= 1`. 프로필 홈이 없는 유일한 배치 처리 노브. |
| `--auto-glossary` | `--no-auto-glossary`와 상호 배타적 | off | 이 실행에 대해 자동 추출된 후보 용어집 프리플라이트를 실행합니다 — 배치 처리 전 1회의 추가 제공자 호출. |
| `--no-auto-glossary` | `--auto-glossary`와 상호 배타적 | off | 프리플라이트를 건너뛰어 옵트인하는 프로필을 오버라이드합니다. |
| `--quiet` | `--verbose`와 상호 배타적 | off | 모든 `transync: ` 라인을 억제하고 트레이싱 바닥을 `off`로 설정합니다. |
| `--verbose` | `--quiet`와 상호 배타적 | off | 성공 시 검증 집계를 출력하고 트레이싱 바닥을 `debug`로 설정합니다. |

재시도 정책 노브 (`max_per_unit_validation_retries`, `max_per_batch_schema_retries`, `max_per_batch_provider_retries`)에 대한 플래그는 없습니다; 내장 값은 `2`, `2`, `1`입니다. 취소에 대한 플래그도 없습니다: `TranslateOptions::cancel`은 라이브러리 기능이며 CLI는 이를 결코 설정하지 않으므로, `translate` 실행 중 Ctrl-C는 깨끗한 취소를 생성하는 대신 프로세스를 종료합니다.

### 출력 대상 (Output destinations)

`--out-dir <dir>`은 고정된 레이아웃을 게시합니다:

```
<dir>/
├── out.md                   번역된 Markdown
├── alignment.json           정렬 맵
├── validation-report.json   이 모드에서 항상 작성됨
├── html/                    6개 파일로 구성된 데모 번들
└── .transync-out-dir        소유권 마커
```

6개 번들 파일 이름은 `index.html`, `source.html`, `target.html`, `alignment.json`, `sync.js`, `purify.min.js`입니다. `--html-out` 디렉터리에 있는 다른 일반 파일은 디렉터리를 외부(foreign) 파일로 만들어 `--force`를 요구합니다; `*.tmp.<pid>` 스테이징 잔여물과 `.transync-publish.lock` 마커는 transync 자체의 것이며 외부 파일로 카운트되지 않습니다.

새로운 `--out-dir` 대상은 하나의 원자적 이름 변경으로 나타납니다. 기존 대상을 대체하는 것은 충돌 시 안전하지만 원자적이지는 **않습니다**: 이전 트리가 먼저 옆으로 이름 변경되므로 두 이름 변경 사이에 보는 무관한 독자는 대상을 전혀 찾을 수 없습니다.

`--out-dir`이 없으면 `--output`과 `--map`이 모두 필요합니다. 어느 하나만으로는 합법적인 호출이 아닙니다.

대상 가드는 두 번 실행됩니다: 유료 번역 비용이 들지 않도록 제공자 호출 전에 한 번 조용히 실행되며; `--out-dir`에 대한 게시 잠금 아래 게시 시점에 다시 실행됩니다. 두 패스 모두 동일한 종료 코드와 동일한 문장을 생성합니다.

### 인자 경계 거부

이들은 CLI 자체의 거부이며, 모두 종료 `1`이고 각각 `transync: <message>`로 출력됩니다:

- `--target-language must not be empty`
- `--source-language must not be empty (use "auto" to detect)`
- `--model must not be empty`
- `--title must not be empty`
- `one of --out-dir, or both --output and --map, is required`
- `--output requires --map (or use --out-dir)`
- `--map requires --output (or use --out-dir)`
- `invalid --base-url: {e}`
- `invalid TRANSYNC_OPENAI_BASE_URL: {e}`
- `OPENAI_API_KEY not set; set it or build --features test-stub-provider`
- `invalid profile TOML: {e}`
- `failed to read --profile {path}: {e}` / `failed to read --system-prompt-file {path}: {e}`
- `--profile {path} is not valid UTF-8` / `--system-prompt-file {path} is not valid UTF-8`

마지막 두 항목의 크기 캡 형제는 `1`이 아니라 종료 `2`입니다: `--profile {path} exceeds the 4194304-byte read limit`.

인자 파서 자체가 결정한 거부 — 알 수 없는 플래그, 충돌하는 쌍, 선언된 범위를 벗어난 값 — 도 종료 `1`이지만 `transync: ` 라인 대신 파서 자체의 사용법 오류를 출력하며, `--quiet`로 이를 억제할 수 없습니다.

### 파서 강제 제약 조건

- 상호 배타적 쌍: `--out-dir` 대 `--output`, `--map`, `--html-out` 각각; `--system-prompt` 대 `--system-prompt-file`; `--auto-glossary` 대 `--no-auto-glossary`; `--quiet` 대 `--verbose`.
- 범위 `n >= 1`: `--target-input-tokens-per-batch`, `--max-units-per-batch`, `--max-concurrent-batches`. `--target-output-tokens`는 범위 검사를 전달하지 않습니다.
- 고정 값 목록: `--table-strategy`는 `whole-block` 및 `row-window-first`만 수용합니다.
- `--output-expansion-factor`는 숫자가 아니거나 유한하지 않거나 0 이하인 경우 파싱 시점에 거부됩니다:
  `` `{s}` is not a number `` 또는
  `` expansion factor must be a positive, finite number, got `{s}` ``.

## 기본값: 내장 기본값 대 내장된 기본 프로필

`--profile`이 없는 실행은 바이너리에 내장된 프로필을 사용하며, 해당 프로필은 여러 라이브러리 내장 항목을 오버라이드합니다. 둘이 다른 경우 프로필이 승리합니다.

| 노브 | 라이브러리 내장값 | 내장된 기본 프로필 | `--profile` 없을 때 유효값 |
|---|---|---|---|
| `target_output_tokens` | 미설정 — 상한 없음 | `8000` | `8000` |
| `output_expansion_factor` | `2.0` | `2.0` | `2.0` |
| `target_input_tokens_per_batch` | `6000` | 미설정 | `6000` |
| `max_units_per_batch` | `32` | `8` | `8` |
| `max_concurrent_batches` | `6` | 프로필 키 없음 | `6` |
| `default_table_strategy` | `whole-block` (없는 키는 여기서 해소됨) | `row-window-first` | `row-window-first` |

`--profile <path>`는 내장된 프로필과 병합되는 대신 이를 대체하므로, 키가 생략된 사용자 지정 프로필은 중간 열이 아니라 **내장(built-in)** 열로 떨어집니다. 따라서 `[constraints].default_table_strategy` 및 `--table-strategy`가 없는 사용자 지정 프로필은 대형 테이블을 분할하지 않습니다.

## 배치 처리 및 동시성

**플래그 > 프로필 > 내장 기본값**에 따라 해결된 프로필 위에 단일 오버레이로 5개 노브가 적용됩니다. 오버레이가 무조건적이므로, 내장 기본값과 동일한 플래그 값도 여전히 프로필을 오버라이드합니다.

| 플래그 | 프로필 키 |
|---|---|
| `--target-output-tokens` | `[batching].target_output_tokens` |
| `--output-expansion-factor` | `[batching].output_expansion_factor` |
| `--target-input-tokens-per-batch` | `[batching].target_input_tokens_per_batch` |
| `--max-units-per-batch` | `[batching].max_units_per_batch` |
| `--table-strategy` | `[constraints].default_table_strategy` |

`--max-concurrent-batches`는 예외입니다: 프로필 홈이 없으며 실행 옵션에 직접 작성됩니다.

**`--target-output-tokens` 및 0.** 플래그는 범위 검증기를 전달하지 않으므로 `0`이 파싱되며 "상한 없음"에 대한 문서화된 센티널입니다: 제공자에게 캡이 전송되지 않고, 출력 인식 패킹이 꺼지며, 위험 시 프리플라이트가 꺼집니다. 내장된 기본 프로필이 항상 `8000`을 설정하므로, 해당 프로필을 취하는 실행이 "상한 없음"을 표현하는 유일한 방법은 `--target-output-tokens 0`입니다; 키를 생략하는 사용자 지정 `--profile`은 처음부터 상한이 없습니다. 별도로, `64`(고정 응답 엔벨로프 예비) 이하의 임의의 0이 아닌 값은 단일 번역 유닛을 위한 공간을 남기지 않습니다; 프로필 로더는 이에 대해 경고하고 미설정으로 정규화하여 상한을 역시 비활성화합니다. `n >= 1` 범위를 가진 3개의 형제 플래그는 대신 파싱 시점에 `0`을 종료 `1`로 거부합니다.

테이블은 상한이 존재하고 유닛의 추정 응답이 패커가 사용하는 동일한 인코더 및 동일한 확장 계수로 측정된 `ceiling - 64`를 초과할 때만 분할됩니다. `--target-output-tokens`를 올리거나 `--output-expansion-factor`를 내리면 분할 임계값과 윈도우 경계가 함께 이동합니다.

## 언어 라벨 규칙

`--source-language`와 `--target-language` 모두 검증된 태그가 아닌 불투명한 라벨을 받습니다. 인자 경계에서 두 가지 정규화가 정확히 한 번 일어납니다:

- 두 라벨 모두에서 둘러싸는 공백이 트림됩니다.
- 인식된 소스 센티널은 정준 철자로 저장됩니다. `auto`는 패딩 및 ASCII 대소문자를 통해 인식되는 하나의 예약된 리터럴이므로, `--source-language ' AUTO '`는 센티널이며 `auto`로 저장됩니다.

다른 것은 접히지(folded) 않습니다. 내부 텍스트, 대소문자 및 비 ASCII 문자가 유지되므로 `--target-language 'Korean (formal, 존댓말)'`은 모델에 그대로 도달하고 `--target-language AUTO`는 일반 라벨입니다. 빈 라벨이나 공백만 있는 라벨은 종료 `1`입니다.

정규화된 값은 컴파일된 시스템 프롬프트, 요청 페이로드, 캐시 키, 정렬 맵, 그리고 번들의 `lang` 및 `dir` 속성이 모두 읽는 내용입니다.

## `--strict-csp` 정책

날인된 정책 원문:

```
default-src 'self'; img-src 'self' data:; script-src 'self' 'unsafe-inline';
style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'
```

`'self'`는 서빙 오리진에 국한되므로, 동일한 문서 루트 아래의 형제 경로가 아닌 다른 호스트를 차단합니다. 원격 이미지는 그 아래에서 렌더링을 멈춥니다; 번들 자체가 필요한 모든 것 — 인라인 스타일, 모듈 스크립트, 3개의 동일 오리진 가져오기, `data:` 이미지 — 은 합법적인 상태를 유지합니다. 실행이 내보내는 번들(`--html-out`, 또는 `--out-dir`의 `html/`)에 적용됩니다. 둘 다 없으면 플래그는 효과가 없으며 실행은 stderr에 이를 출력합니다.

`<meta>`로 전달된 정책이 이를 무시하므로 `frame-ancestors`, `sandbox`, `report-uri`는 없습니다.

## `transync serve`

렌더링된 번들 디렉터리를 위한 루프백 정적 파일 서버입니다. 서빙만 수행하고 다른 것은 하지 않습니다: 업로드 없음, 디렉터리 목록 없음, 실행 없음, 프록시 없음.

| 플래그 | 필수 여부 | 기본값 | 의미 |
|---|---|---|---|
| `--rendered <dir>` | 예 | — | 서빙할 디렉터리, 보통 `--html-out` 번들. 시작 시 한 번 정준화되므로 심볼릭 링크된 디렉터리는 가리키는 디렉터리로 서빙됩니다. |
| `--port <u16>` | 아니오 | `7470` | TCP 포트. `0`은 OS에 사용 가능한 포트를 요청하며 바인딩된 주소와 함께 출력됩니다. |
| `--bind <addr>` | 아니오 | `127.0.0.1` | 바인딩할 주소, IP 주소로 파싱됨. IP 주소가 아닌 값은 종료 `1`입니다. 비 루프백 주소는 경고를 출력합니다. |

`serve`에는 `--quiet` 및 `--verbose`가 없습니다. 항상 기본 트레이싱 바닥(`warn`)에서 실행되며 자체 진단은 `transync: `가 아니라 `transync serve: ` 접두사가 붙습니다.

### 시작 및 종료 라인

stderr에 순서대로:

```
transync serve: listening on http://127.0.0.1:7470/ — serving /abs/path/to/dir
transync serve: WARNING: <ip> is not a loopback address — everything under <root> is reachable from other machines on this network.
transync serve: press Ctrl-C to stop.
```

경고 라인은 바인딩된 주소가 루프백 주소가 아닐 때만 나타납니다. Ctrl-C는 해당 기능을 가진 플랫폼의 `SIGTERM`(서버를 중지하는 수퍼바이저가 후자를 전송함)과 마찬가지로 수용 루프를 중지합니다; 그런 다음 서버는 `transync serve: shutting down.`을 출력하고 2초 동안 인플라이트 연결을 비운 다음 종료 `0`을 합니다. 유예 기간 이후에도 연결이 남아 있으면 `transync serve: N connection(s) still open after 2s; closing anyway.`를 출력하고 연결을 중단합니다.

수용 오류는 `transync serve: accept failed: {err}`를 출력하고 치명적이지 않습니다; 루프는 50 ms 백오프 후 속행합니다.

### HTTP 계약

`GET` 및 `HEAD`만 지원합니다. `HEAD` 응답은 헤드만 전달하고 본문은 전달하지 않습니다.

| 상태 | 원인 |
|---|---|
| `200 OK` | 서빙되는 루트 내부의 일반 파일. |
| `400 Bad Request` | 요청 라인이 `HTTP/` 버전으로 끝나는 3개 토큰이 아님; 대상이 origin-form이 아님; 퍼센트 이스케이프가 잘렸거나 16진수가 아님; 디코딩된 세그먼트가 유효한 UTF-8이 아님; 세그먼트가 `/`, `\` 또는 NUL을 포함하는 것으로 디코딩됨. |
| `403 Forbidden` | 세그먼트가 `.` 또는 `..`로 디코딩됨; 또는 정준화된 경로가 서빙되는 루트 외부로 해소됨 (심볼릭 링크 이탈을 포착함). |
| `404 Not Found` | 해당 경로에 아무것도 없거나 일반 파일이 아님 — `index.html`이 없는 디렉터리, 장치, 소켓. |
| `405 Method Not Allowed` | `GET` 또는 `HEAD` 이외의 메서드. 응답은 `Allow: GET, HEAD`를 전달합니다. |
| `408 Request Timeout` | 요청 헤드가 헤드 타임아웃 내에 도착하지 않음. |
| `431 Request Header Fields Too Large` | 요청 헤드가 헤드 상한을 초과함. |

거부는 거부하며(reject); 트래버설을 루트로 다시 클램프하지 않습니다. 거부된 요청은 연결 상의 HTTP 상태이며 프로세스 종료가 아닙니다 — 서버는 서빙을 계속합니다.

모든 응답은 다음 헤더를 전달합니다:

| 헤더 | 값 |
|---|---|
| `Content-Type` | 아래 확장자 테이블로부터. |
| `Content-Length` | 실제로 작성된 본문 길이(메모리 내 제한 이하의 본문) 또는 열린 핸들의 길이. |
| `Connection` | `close` |
| `X-Content-Type-Options` | `nosniff` |
| `Cache-Control` | `no-store` |

keep-alive는 없습니다: 연결당 정확히 하나의 요청만 읽습니다.

### 콘텐츠 타입

콘텐츠 스니핑이 아닌 파일 확장자로만 결정됩니다. 확장자는 대소문자를 구분하지 않고 일치되며 타입은 **정준** 경로에서 가져오므로 심볼릭 링크가 파일 라벨을 재지정할 수 없습니다.

| 확장자 | `Content-Type` |
|---|---|
| `.html`, `.htm` | `text/html; charset=utf-8` |
| `.js`, `.mjs` | `text/javascript; charset=utf-8` |
| `.json`, `.map` | `application/json; charset=utf-8` |
| `.css` | `text/css; charset=utf-8` |
| `.md` | `text/markdown; charset=utf-8` |
| `.txt` | `text/plain; charset=utf-8` |
| `.svg` | `image/svg+xml` |
| `.png` | `image/png` |
| `.wasm` | `application/wasm` |
| 기타 모든 확장자, 또는 확장자 없음 | `application/octet-stream` |

### 경로 해소

요청 대상은 디코딩되기 **전**에 `/` 세그먼트로 분할되며, 각 세그먼트는 자체적으로 퍼센트 디코딩됩니다. 단독 `/` 및 `/`로 끝나는 대상은 지정된 디렉터리의 `index.html`로 해소됩니다. 해소된 경로는 정준화된 후 정준 루트에 대해 검사됩니다; 그 후에야 열립니다.

### 제한

| 제한 | 값 |
|---|---|
| 요청 헤드 크기 | 8 KiB (초과 시 `431`) |
| 요청 헤드 도착 시간 | 15 s (초과 시 `408`) |
| 인플라이트 연결 | 128; 수용 루프는 하나가 끝날 때까지 대기합니다 |
| 수용 오류 백오프 | 50 ms |
| 메모리로 읽어들이는 응답 | 8 MiB 이하; 더 큰 파일은 스트리밍됨 |
| 종료 유예 기간 | 2 s |

### 하지 않는 작업

- **디렉터리 목록 없음** (두 철자 모두). `index.html`이 없는 디렉터리는 `404`입니다; 내부 파일은 이름으로 계속 도달 가능합니다.
- **TLS 없음, HTTP 범위 없음, 조건부 요청 없음**, 비 루프백 배포 강화 없음. 이들은 결정에 의해 범위를 벗어납니다.
- **문서화된 하나의 잔여 레이스(race).** 경로는 정준화되고 검사된 후 열립니다. 다른 로컬 프로세스가 검사와 열기 사이에 항목을 대체할 수 있습니다. 윈도우는 하나의 시스템 호출 너비이며 열린 경로는 링크가 포함되어 있지 않음이 방금 확인된 경로이므로, 이를 활용하려면 서빙되는 디렉터리 내부의 로컬 쓰기 권한이 필요합니다. 이를 완전히 닫으려면 `openat`/`O_NOFOLLOW` 순회가 필요하지만 이 서버는 이를 수행하지 않습니다.

## 종료 코드

| 코드 | `transync translate` | `transync serve` |
|---:|---|---|
| 0 | 성공. 또한 `--help` 및 `--version`. | 종료 신호가 비우기 후 실행 중인 서버를 중지함. |
| 1 | 인자 파서가 argv를 거부했거나, `--profile` 로드에 실패했거나, CLI 자체 경계 거부 중 하나가 트리거됨. | 인자 파서가 IP 주소가 아닌 `--bind` 값을 포함하여 argv를 거부함. |
| 2 | 입력 읽기 또는 파싱 실패: `--input` 누락, 읽을 수 없음, 유효한 UTF-8이 아님, 또는 `--max-input-bytes`보다 큼; 고정 4 MiB 캡을 초과한 `--profile` 또는 `--system-prompt-file`; 깊이 128에서의 블록 중첩 거부를 포함한 파싱 오류. | `--rendered`를 정준화할 수 없거나, 디렉터리가 아니거나, 읽을 수 없음. |
| 3 | 모든 번역 가능한 유닛이 원문으로 폴백됨. 모든 출력은 여전히 작성됨. 실행에 최소 하나의 유닛이 있었을 때만 트리거됨. | 도달 불가능. |
| 4 | 쓰기 실패 — `could not write --html-out bundle to {dir}: {source}` 또는 `could not write outputs: {e}`. | 도달 불가능. |
| 5 | 잔여물: 이 빌드가 더 이상 분류할 수 없는 제공자 또는 파이프라인 실패, 또는 정렬 맵 / 검증 보고서 직렬화 실패. | 주소를 바인딩할 수 없거나(포트 사용 중, 주소가 로컬에 할당되지 않음), 바인딩된 주소를 다시 읽을 수 없음. |
| 6 | 제공자가 **구성 방식**에 대해 실행을 거부함 — 거부된 자격 증명, 제공자가 직접 거부한 요청, 소진된 출력 상한, 또는 초과된 컨텍스트 윈도우. | 도달 불가능: `serve`에는 제공자가 없음. |
| 7 | 제공자가 **이 문서의 콘텐츠**를 거부함 — 콘텐츠 정책이 생성을 중지했거나 모델이 거절하고 그렇게 밝힘. | 도달 불가능. |

`6`과 `7`은 스크립트가 분기할 수 있는 두 가지 코드입니다. 호출자가 해야 하는 작업이 다르기 때문입니다: `6`에는 구성 해결 방법이 있지만 `7`에는 없습니다 — 재시도는 동일한 페이로드를 다시 제출하므로 동일한 콘텐츠가 다시 거부됩니다. 둘 다 출판되기 전에 중단되므로 출력이 작성되는 `3`과 구별됩니다.

코드는 추가 전용(append-only)입니다. `6`과 `7`은 원인을 `5`에서 이동하여 추가되었습니다; `0`–`5` 코드는 여전히 항상 의미했던 바를 의미합니다.

**0이 아닌 종료와 함께 출력되는 내용.** 인자 파싱 후 실패하는 `translate` 실행은 stderr에 하나의 `transync: <reason>` 라인을 출력하며, `--quiet`로 억제됩니다. 파서 수준 실패는 대신 파서 자체의 형식으로 자체 오류를 출력하며 어떤 플래그도 이를 억제할 수 없습니다. `serve`에는 `--quiet`가 없으며 실패 라인에는 `transync serve: ` 접두사가 붙습니다 — 예: `transync serve: cannot bind {addr}: {err}` 및 `transync serve: cannot serve {path}: {err}`.

## 환경 변수

| 변수 | 기본값 | 효과 |
|---|---|---|
| `OPENAI_API_KEY` | — | 라이브 제공자에 필요함. 환경에서 읽히며 어디에도 결코 작성되지 않음. |
| `TRANSYNC_OPENAI_MODEL` | `gpt-5-chat-latest` | `--model`이 없을 때의 모델 ID. 빈 값 또는 공백만 있는 값은 무시되고 기본값이 적용됨. |
| `TRANSYNC_OPENAI_BASE_URL` | `https://api.openai.com` | `--base-url`이 없을 때의 제공자 엔드포인트 — OpenAI 호환 프록시, 게이트웨이 또는 로컬 서버. 빈 값은 무시됨. 파싱 불가능한 값은 종료 `1`. |
| `TRANSYNC_OPENAI_API` | 모델 주도 | `chat` 또는 `responses`; 엔드포인트 디스패치 휴리스틱의 명시적 오버라이드. 어댑터 생성 시 한 번 읽힘. |
| `RUST_LOG` | 미설정 | 플래그 도출 트레이싱 바닥을 대체하기보다 그 **위에** 레이어링되므로 `RUST_LOG=transync::pipeline=trace`는 하나 대상을 넓히고 다른 모든 것은 바닥에 둡니다. 바닥: `--quiet`는 `off`이며 `RUST_LOG`를 전혀 참조하지 않음; 플래그 없음은 `warn`; `--verbose`는 `debug`. 파싱 불가능한 지시어는 `transync: ignoring unparsable RUST_LOG directive {directive}: {e}`를 출력하고 건너뜀; 변수의 나머지는 여전히 적용됨. |

라이브러리 `tracing` 레코드는 타임스탬프 없이 명령 자체의 라인과 동일한 stderr에 `LEVEL target: message`로 출력됩니다.

두 번째 제공자 크레이트인 `transync-anthropic`이 워크스페이스에 존재하며 자체 `ANTHROPIC_API_KEY`, `TRANSYNC_ANTHROPIC_MODEL` 및 `TRANSYNC_ANTHROPIC_BASE_URL`을 읽습니다 — 그러나 제공자를 선택하는 CLI 플래그가 없으며 바이너리는 OpenAI 어댑터에만 의존하므로 세 변수 중 어느 것도 `transync` 호출에 영향을 미치지 않습니다.

4개 추가 변수는 `test-stub-provider` cargo 기능을 탑재한 빌드에만 존재하며 릴리스 바이너리에는 없습니다: `TRANSYNC_STUB_ECHO_PATH`, `TRANSYNC_STUB_MODE`, `TRANSYNC_STUB_DETECT`, `TRANSYNC_STUB_GLOSSARY`.

## 성공적인 실행 시 권고 stderr 라인

`0`으로 종료되는 실행도 `transync: note: …` 라인을 출력할 수 있습니다. 각 라인은 권고 사항입니다: 어느 것도 종료 코드를 변경하지 않으며 게시된 바이트의 정합성 여부를 변경하지 않습니다. `--quiet`는 이를 모두 억제합니다.

| 라인 | 의미 |
|---|---|
| `note: <skipped source node>` | 번역된 Markdown으로 살아남지만 일반적인 번역 콘텐츠가 아닌 최상위 소스 노드 — 프론트매터, 각주 정의, 번역할 내용이 없는 html 블록, 또는 세그먼트 추출에 실패한 html 블록. |
| `note: waiting for another transync run to finish publishing into {dir}` | 다른 실행이 해당 디렉터리에 대한 게시 잠금을 보유함. 이 실행은 대기하며 둘 다 실패하지 않음. |
| `note: {dir} holds {n} staging temp(s) from other transync runs (e.g. {first}) — kept in case a run is still staging into them; delete them by hand once no transync run is active` | 다른 프로세스의 PID를 전달하는 스테이징 잔여물은 자동으로 회수되지 않음. |
| `note: could not flush the directory {dir} to disk ({e}); the published file CONTENTS are durable, but the directory entries naming them may not survive a crash — re-run the publication if the machine goes down before the filesystem catches up` | 디렉터리를 디스크로 플러시할 수 없었거나 시도하기 위해 열 수조차 없었음. |
| `note: could not clean up {n} {what} after a failed publication (e.g. {path}: {e}); the residue is inert — delete it by hand once no transync run is active` | *실패한* 게시 후 최선 노력 정리로 잔여물이 남음. `{what}`은 `staged temp file(s)` 또는 `directory level(s) this run created`. 얼마나 많은 항목이 실패했든 한 번 출력되며, 실행의 실제 오류 **옆에** 출력됨. |
| `note: could not carry the existing permissions of {path} over to its replacement ({e}); the published file keeps this run's default mode instead` | Unix 전용. |
| `note: --strict-csp has no effect without --html-out or --out-dir (this run emits no HTML bundle)` | 플래그가 전달되었으나 번들이 내보내지지 않음. |
| `could not open --cache-dir {dir}: {e}; continuing with a fresh in-memory cache (this run will not reuse or persist anything)` | 디스크 캐시를 열 수 없음. 실행은 속행되어 모든 것을 번역함. |

두 개 추가 라인은 노트가 아닌 경고이며 종료 코드를 변경하지 않습니다:

- `warning: auto-glossary: extraction failed ({diagnostic}); static glossary only`
  — `--quiet`를 제외하고 무조건 출력되므로 `RUST_LOG` 지시어로 이를 숨길 수 없습니다.
- `warning: {n} unit(s) estimated over the per-batch output ceiling (each named on the transync::pipeline warning channel and in --validation-report) — raise --target-output-tokens, lower --output-expansion-factor, or split the source block` — 플래그 지정된 유닛 중 최소 하나가 테이블 블록일 때 "or split" 앞에 `, --table-strategy row-window-first (tables only)`가 삽입됨. 얼마나 많은 유닛이 플래그 지정되었든 실행당 정확히 하나의 라인.

`--verbose`는 집계 라인 `transync: total_units=N translated=N preserved=N partial=N fallback=N retried=N`을 추가하고, 자동 용어집 프리플라이트가 실행되었을 때 하나의 `transync: auto-glossary: …` 요약 라인을 추가합니다.

프로필 로드 경고는 CLI에 의해 다시 출력되지 않습니다. 기본 `warn` 바닥에서 `transync::profile` 트레이싱 대상을 통해서만 stderr에 도달합니다.

## 관련 페이지

- [CLI로 문서를 번역하는 방법](../../../how-to/user/ko/translate-a-document-with-the-cli.md)
- [실행 간 웜 캐시를 재사용하는 방법](../../../how-to/user/ko/reuse-a-warm-cache-across-runs.md)
- [번역 실행을 진단하는 방법](../../../how-to/user/ko/diagnose-a-translation-run.md)
- [번역 프로필을 작성하는 방법](../../../how-to/user/ko/write-a-translation-profile.md)
- [데모 번들을 서빙하는 방법](../../../how-to/operator/ko/serve-the-demo-bundle.md)
- [wasm 데모를 빌드하는 방법](../../../how-to/operator/ko/build-the-wasm-demo.md)
- [프로필 TOML 스키마](./profile-toml-schema.md)
- [정렬 맵 스키마](../../developer/ko/alignment-map-schema.md)
- [배치가 섹션 경계에서 멈추는 이유](../../../explanation/user/ko/why-batches-stop-at-section-boundaries.md)
