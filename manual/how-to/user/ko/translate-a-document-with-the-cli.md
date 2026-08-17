---
type: How-To Guide
title: CLI로 마크다운 문서를 번역하는 방법
description: `transync translate`를 종단 간으로 실행하여 번역된 문서와 탐색 가능한 이중 패널 데모 번들을 생성한 다음 `transync serve`로 엽니다.
tags: [cli, translation, serve, caching, SCN-12, SCN-13]
audience: user
language: ko
sources:
  - { id: en-source, resource: manual/how-to/user/en/translate-a-document-with-the-cli.md }
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:01Z
synced_hash: 0af6d97843e9a16cbd46c649a1336947e1e7436dd7513e41bf3e11ba7390be89
---
# CLI로 마크다운 문서를 번역하는 방법

`transync translate`로 GFM 마크다운 파일을 번역하고 그 결과를 브라우저에서 스크롤 동기화된 원문/대상 데모 페이지로 확인합니다.

## 사전 요구 사항

- `PATH`에서 사용할 수 있거나 체크아웃한 소스에서 `cargo build --workspace`로 빌드된 `transync` 바이너리.
- 라이브 OpenAI 공급자를 위해 내보낸 `OPENAI_API_KEY` (또는 동일한 키를 허용하는 프록시를 가리키는 호환되는 `--base-url`).
- 손에 쥔 원문 마크다운 파일의 경로 및 대상 언어 레이블 (예: `ko`, `ja-JP`).

그 외에는 아무것도 필요하지 않습니다. `transync serve`가 데모 번들을 제공하므로 별도의 정적 HTTP 서버가 필요하지 않습니다.

## 1. 출력 레이아웃 선택

`transync translate`는 상호 배타적인 두 가지 레이아웃 중 하나로 출력 세트를 작성합니다 — `--out-dir`을 `--output`, `--map`, 또는 `--html-out`과 함께 전달하면 `clap` 인자 오류 (종료 코드 `1`)가 발생합니다:

- **개별 경로** — `--output` 및 `--map` (함께 필요) 및 탐색 가능한 데모 번들을 위한 선택적 `--html-out` 디렉토리.
- **`--out-dir`** — 번역된 마크다운, 정렬 맵, 검증 보고서, 및 데모 번들을 항상 함께 받는 하나의 디렉토리.

특정 위치(예: 문서 트리에 커밋됨)에 번역된 마크다운이나 정렬 맵을 원할 때는 개별 경로를 사용하세요. "모든 것을 한 곳에 줘"를 원할 때는 `--out-dir`을 사용하세요.

## 2. 개별 경로 및 데모 번들로 번역하기

```bash
export OPENAI_API_KEY=sk-...

transync translate \
  --input README.md \
  --output README.ko.md \
  --map README.alignment.json \
  --html-out dist/demo \
  --target-language ko
```

- 이 형태에서는 `--output`과 `--map`이 함께 필요합니다.
- `--html-out dist/demo`는 선택 사항이지만 탐색 가능한 번들(`index.html`, `source.html`, `target.html`, `alignment.json`, `sync.js`, `purify.min.js`)을 생성하는 옵션입니다.
- 번들 자체의 6개 파일은 반복 실행 시 덮어써집니다; `dist/demo`에 나란히 작성하려는 관련 없는("외래") 파일이 포함되어 있는 경우에만 `--force`를 추가하세요. 그 거부 검사는 공급자가 호출되기 전에 수행되므로 외래 파일로 인한 중단은 비용이 들지 않습니다.
- 유닛별 시도 로그(거부 이유, 공급자 경고)를 자체 파일로 원하면 `--validation-report path/to/report.json`을 추가하세요; 이는 `--out-dir` 아래에서는 효과가 없습니다 (4단계 참조).

0이 아닌 종료 코드는 열어볼 결과가 있기 전에 주의가 필요한 사항이 있음을 의미합니다 — 출력된 `transync: <reason>` 라인을 확인하세요. (종료 코드 `3`은 예외입니다: 모든 유닛이 원문으로 폴백된 경우에도 출력은 작성됩니다 — 여기에 해당하는 경우 [번역 실행을 진단하는 방법](./diagnose-a-translation-run.md)을 참조하세요.)

`0`으로 종료되는 실행에서도 권고성 `transync: note: …` 라인이 출력될 수 있습니다 — 다른 실행의 게시 잠금 뒤에서 대기 중, 다른 실행이 남긴 임시 파일 준비, 디렉토리의 디스크 플러시 실패 등. 그 중 어느 것도 종료 코드나 게시된 바이트를 변경하지 않습니다.

## 3. `transync serve`로 번들 보기

```bash
transync serve --rendered dist/demo
```

기본적으로 `127.0.0.1:7470`에 바인딩되며 stderr에 다음을 출력합니다:

```
transync serve: listening on http://127.0.0.1:7470/ — serving /abs/path/to/dist/demo
transync serve: press Ctrl-C to stop.
```

해당 URL을 열고 두 패널이 렌더링되고 둘 중 하나를 스크롤할 때 동기화 상태를 유지하는지 확인하세요. Ctrl-C는 서버를 정지하고 `0`으로 종료합니다. 포트가 이미 사용 중인 경우 `--port 0`은 OS에 사용 가능한 포트를 요청하며 `listening on` 라인에 새로 얻은 포트가 보고됩니다.

서버가 진정으로 필요합니다: 번들 쉘은 `file://` URL에서는 작동하지 않는 자체 옆의 `source.html`, `target.html` 및 `alignment.json`을 가져옵니다. 루프백이 아닌 `--bind` 사례, 패널이 실제로 마운트되었는지 확인하는 방법, 그리고 전체 상태 및 MIME 동작에 대해서는 [데모 번들을 제공하는 방법](../../operator/ko/serve-the-demo-bundle.md)을 참조하세요.

## 4. 대안: `--out-dir`을 사용한 단발성 실행

```bash
export OPENAI_API_KEY=sk-...

transync translate \
  --input README.md \
  --out-dir dist/out \
  --target-language ko
```

이는 다음을 게시합니다:

```
dist/out/
├── out.md                   # 번역된 마크다운
├── alignment.json           # 정렬 맵
├── validation-report.json   # --validation-report 여부와 상관없이 항상 여기에 작성됨
└── html/                    # --html-out과 동일한 6개 파일 데모 번들
```

`dist/out`이 아니라 `html/` 하위 디렉토리를 `serve`가 가리키도록 설정하세요 — 최상위 레벨에는 `index.html`이 없습니다:

```bash
transync serve --rendered dist/out/html
```

이전 transync 실행에서 생성된 기존 `dist/out`은 그 자리에서 다시 게시됩니다; 디렉토리에 `{out.md, alignment.json, validation-report.json, html}` 외의 다른 파일이 포함되어 있다면 `--force`를 전달하여 교체하세요.

해당 재게시에는 두 가지 정직한 한계가 존재합니다:

- **새로운 대상은 원자적으로 나타납니다; 기존 대상을 대체하는 것은 그렇지 않습니다.** 새 트리가 제자리에 이름 변경되기 전에 이전 트리의 이름이 옆으로 변경되므로 게시는 충돌 안전(crash-safe)하지만 원자적이지 않습니다: 두 이름 변경 사이에 조회를 수행하는 관련 없는 리더는 `dist/out`을 전혀 찾지 못합니다. 라이브 소비자가 재게시 중인 디렉토리를 가리키게 하지 마세요.
- **`.transync-publish.lock` 마커가 뒤에 남습니다.** transync는 게시하는 모든 디렉토리에 해당 0바이트 파일을 잠그고 결코 언링크하지 않습니다. 의도적인 동작이므로 그대로 두세요; 실패로 인한 잔재가 아닙니다.

## 5. 다음 실행을 저렴하게 만들기

기본적으로 실행 간에 보존되는 것은 없습니다 — 각 호출은 새로운 메모리 내 캐시를 구축하므로 동일한 문서를 두 번 번역하면 공급자에게 두 번 비용을 지불하게 됩니다. `--cache-dir <dir>`을 추가하면 실행이 해당 위치에 `transync-cache.jsonl` 로그를 유지하므로 동일한 프로필, 모델 및 언어 하에서 동일한 문서에 대한 두 번째 실행은 실제로 변경된 내용만 다시 전송합니다:

```bash
transync translate \
  --input README.md \
  --out-dir dist/out \
  --target-language ko \
  --cache-dir .transync-cache
```

해당 디렉토리를 운용하기 위한 레시피 — 항목을 무효화하는 요소, 크기 예산의 역할, 한 번에 하나의 실행만 작성할 수 있는 이유 — 는 [실행 간 따뜻한 캐시 재사용하기](./reuse-a-warm-cache-across-runs.md)를 참조하세요.

## 다음 단계

- 전체 플래그 목록, 기본값, 환경 변수 및 종료 코드: [CLI 참조](../../../reference/user/en/cli.md).
- 0이 아닌 코드 종료 실행 또는 출력이 잘못되어 보이는 경우: [번역 실행을 진단하는 방법](./diagnose-a-translation-run.md).
- 번역 스타일 제어, 보존된 용어 및 구조 정책: [프로필 TOML을 작성하는 방법](./write-a-translation-profile.md).
