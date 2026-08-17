---
type: Tutorial
title: transync 시작하기
description: 깨끗한 체크아웃에서 transync를 빌드하고, 테스트를 실행하며, API 키 없이 샘플 문서를 번역하고, 브라우저에서 렌더링된 두 패널이 동기화되어 스크롤되는 것을 확인합니다.
tags: [getting-started, cli, serve, SCN-12, SCN-13]
audience: operator
language: ko
generated:
  by: gemini/gemini-3.6-flash
  at: 2026-08-12T11:12:02Z
sources:
  - { id: en-source, resource: manual/tutorials/operator/en/getting-started.md }
synced_hash: 88629671ec700520715b6086bfb603a435810bbe3d41ed3a3ede41fd09e5da75
---
# transync 시작하기

이 튜토리얼을 마칠 때쯤이면 소스에서 transync를 빌드하고, 테스트를 실행하고, 샘플 문서를 번역하고, 브라우저에서 렌더링된 두 패널(원문 및 번역문)이 동기화되어 스크롤되는 것을 관찰할 수 있게 됩니다. 이 모든 것은 오프라인으로 실행됩니다: API 키, 제공자 계정, 추가 도구가 필요하지 않습니다.

## 시작하기 전에

- **Rust 1.89 이상** (`rustc --version`). 라이브러리 크레이트는 1.88에서 빌드되지만, `transync-cli`는 출력을 게시할 때 OS 파일 잠금을 취하며 해당 API는 1.89부터 안정적입니다.
- **git**, 그리고 터미널.

Linux 및 macOS에서 테스트되었습니다; Windows에서도 작동해야 하지만 검증되지 않았습니다. 다른 것은 필요하지 않습니다 — transync는 자체적으로 빌드, 번역 및 결과를 제공합니다.

## 1. 클론 및 빌드

```bash
git clone https://github.com/QuietJoon/transync
cd transync
cargo build --workspace
```

클린 빌드는 1~2분 정도 걸리며, 대부분 `comrak`(Markdown 파서)과 `reqwest`/`rustls`(HTTP 스택)입니다. 빌드는 오류 없이 `Finished` 줄로 끝납니다.

이후의 모든 명령은 저장소 루트에서 실행됩니다.

## 2. 테스트 실행

```bash
cargo test --workspace
```

이 명령은 `crates/transync/tests/scenarios/` 아래의 엔드투엔드 시나리오 수트(헤딩, 테이블, 코드 블록, 중첩 리스트, 검증, 재시도, 폴백, 정렬) — 인프로세스 모의 번역기에 의해 구동됨 — 및 정적 서버 자체 수트를 포함하여 모든 크레이트의 테스트를 빌드하고 실행합니다. 원격 제공자를 호출하는 것은 없습니다. 각 테스트 바이너리는 다음과 같은 줄로 끝납니다:

```
test result: ok. …
```

하나의 수트는 의도적으로 해당 실행 외부에 있습니다. CLI 자체의 엔드투엔드 수트인 `crates/transync-cli/tests/cli_smoke.rs`는 `test-stub-provider` 기능이 활성화된 경우에만 컴파일되며 해당 기능은 기본적으로 꺼져 있습니다 — 따라서 `--workspace`만으로는 바이너리의 번역/제공 경로를 건드리지 않습니다. 다음 명령도 실행하세요:

```bash
cargo test -p transync-cli --features test-stub-provider
```

저장소의 하드 게이트인 `scripts/smoke.sh`는 정확히 이러한 이유로 두 명령을 별도의 단계로 실행합니다. 이 두 번째 명령은 또한 다음 단계에 필요한 바이너리를 이미 빌드된 상태로 남겨둡니다.

## 3. 샘플 문서 번역

`test-stub-provider` 기능은 라이브 OpenAI 클라이언트를 인프로세스 에코 번역기로 교체하므로 이 실행에는 API 키가 필요하지 않습니다. 에코는 각 유닛의 원문 텍스트를 변경 없이 돌려줍니다: 사용자가 진행하는 테스트는 문장이 아니라 파이프라인 — 파싱, 유닛 생성, 배치 처리, 검증, 재생성, 렌더링 — 입니다.

```bash
mkdir -p /tmp/transync-demo
cargo run -p transync-cli --features test-stub-provider --quiet -- translate \
  --input crates/transync/tests/fixtures/scn-14-full.md \
  --output /tmp/transync-demo/out.md \
  --map /tmp/transync-demo/out.json \
  --html-out /tmp/transync-demo/html \
  --target-language ko
```

실행이 종료 코드 `0`으로 종료되고 stderr에 한 줄을 출력합니다:

```
transync: note: html block html-0017 contains no translatable text: it is preserved verbatim and live-rendered (possibly visually empty)
```

이것은 실패가 아니라 성공 시의 참고 사항입니다. 픽스처는 텍스트가 없는 순수 HTML 블록으로 파싱되는 단독 `</details>` 줄로 끝납니다; transync는 이를 침묵 속에 삭제하는 대신 보존된 내용을 보고합니다. 위 명령의 `--quiet`는 `cargo`에 속하며 빌드 로그만 숨깁니다 — transync는 `--` 뒤에 올 수 있는 자체 `--quiet`를 가지고 있으며, 이와 같은 참고 사항을 억제하는 것은 해당 플래그입니다. [번역 실행 진단하기](../../../how-to/user/ko/diagnose-a-translation-run.md) 가이드에서 어휘의나머지 부분을 확인할 수 있습니다.

이제 생성된 항목을 확인하세요:

```bash
ls /tmp/transync-demo/html
```

```
alignment.json  index.html  purify.min.js  source.html  sync.js  target.html
```

이 6개 파일이 브라우저에서 볼 수 있는 데모 번들입니다. 그 옆에 `/tmp/transync-demo/out.md`는 재생성된 Markdown이고 `/tmp/transync-demo/out.json`은 두 문서를 블록 단위로 묶는 정렬 맵입니다.

## 4. 번들 제공 및 패널 동기화 관찰

번들의 `index.html`은 자체 옆에서 `source.html`, `target.html` 및 `alignment.json`을 가져오므로 HTTP 출처가 필요합니다 — 디스크에서 파일을 직접 열면 작동하지 않습니다. transync가 서버를 제공합니다:

```bash
cargo run -p transync-cli --features test-stub-provider --quiet -- serve \
  --rendered /tmp/transync-demo/html
```

`--features` 플래그는 cargo가 이미 빌드한 바이너리를 재사용하도록 하기 위해 반복될 뿐입니다; `serve` 자체는 해당 기능을 사용하지 않습니다. 서버는 `127.0.0.1:7470`(루프백이므로 이 시스템에서만 도달 가능)을 바인딩하고 다음을 출력합니다:

```
transync serve: listening on http://127.0.0.1:7470/ — serving …/transync-demo/html
transync serve: press Ctrl-C to stop.
```

첫 번째 줄의 끝 부분은 실제 경로로 해석된 번들 디렉터리입니다.

`http://127.0.0.1:7470/`을 여세요. 두 패널이 나란히 나타납니다. **어느 한 패널을 스크롤하면** 다른 패널이 따라와 약 0.25초 만에 일치하는 블록에 정착합니다. 스크롤 비율이 아닌 블록 ID를 추적하고 있습니다: 해당 대응 관계는 Rust에서 한 번 계산되고 브라우저에서 결코 다시 도출되지 않으며, 이것이 이 프로젝트가 안정적으로 수행하기 위해 존재하는 기능입니다. [아키텍처 개요](../../../explanation/developer/ko/architecture-overview.md)에서 해당 구분이 전체 디자인인 이유를 설명합니다.

작업이 끝나면 터미널에서 Ctrl-C를 누르세요. 서버가 수락을 중지하고 진행 중인 연결이 완료될 때까지 2초를 부여한 후 `0`으로 종료됩니다.

이제 클린 체크아웃 상태에서 transync의 핵심 보장을 빌드, 테스트, 번역, 제공하고 관찰했습니다.

## 다음 단계

- **라이브 모델로 직접 만든 문서 번역하기:**
  [CLI로 문서 번역하는 방법](../../../how-to/user/ko/translate-a-document-with-the-cli.md)
  — 해당 경로에는 `OPENAI_API_KEY`가 필요합니다.
- **프로필로 번역 형성하기:**
  [번역 프로필 작성하는 방법](../../../how-to/user/ko/write-a-translation-profile.md).
- **다른 포트 또는 다른 머신으로 번들 제공하기:**
  [데모 번들 제공하는 방법](../../../how-to/operator/ko/serve-the-demo-bundle.md).
- **모든 플래그 및 종료 코드:**
  [CLI 참조](../../../reference/user/en/cli.md).
