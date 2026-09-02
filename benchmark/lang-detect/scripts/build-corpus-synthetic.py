#!/usr/bin/env python3
"""Build the committed synthetic corpus in `corpus/`.

English is real technical prose sliced out of this repository's own documents.
Korean is AUTHORED — written for this benchmark, not sampled from real agent
output — which is the honest limit of this corpus: because all candidates see
identical input the RANKING it produces is sound, but the absolute flip points
are indicative only. `scripts/build-corpus-real.py` is the one that settles them.

    python3 scripts/build-corpus-synthetic.py
"""
import re, os, random
random.seed(11)
ROOT=os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT=os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),"corpus")
os.makedirs(OUT, exist_ok=True)
def words(t): return [w for w in re.split(r"\s+", t) if w]

en=[]
for rel in ["CHANGELOG.md","docs/architecture/contracts.md","docs/Developer_Guide.md"]:
    p=os.path.join(ROOT,rel)
    if not os.path.exists(p): continue
    t=open(p,encoding="utf-8").read()
    t=re.sub(r"```.*?```","",t,flags=re.S); t=re.sub(r"`[^`]*`","",t)
    t=re.sub(r"[|#*_>\-\[\]()]+"," ",t); t=re.sub(r"https?://\S+"," ",t)
    en+=[w for w in words(t) if re.match(r"^[A-Za-z][A-Za-z'\.,;:]*$",w)]

KO=words("""이 변경은 문서 전반에 걸쳐 남아 있던 오래된 설명을 정리합니다 스캐너가 외부 콘텐츠 안에서
원시 텍스트 상태로 진입하던 문제를 고쳤고 그 결과 브라우저가 실제로 해석하는 방식과 동일하게
동작하게 되었습니다 검증 계층은 이제 번역된 페이로드에도 원본과 같은 중첩 깊이 제한을 적용합니다
정렬 맵의 행이 앵커 여부를 결정하므로 렌더러와 정렬 계층이 서로 다른 판단을 내릴 수 없습니다
이 작업은 여러 티켓에 걸쳐 진행되었으며 각 항목은 별도의 기록으로 남았습니다 테스트는 전체
워크스페이스에서 통과했고 실패한 항목은 없습니다 빈 요소 규칙은 콘텐츠에서만 적용되며 외부
콘텐츠에서는 일반 요소로 취급됩니다 작성자가 직접 쓴 종료 태그가 삭제되던 동작은 더 이상
발생하지 않습니다 이러한 결정은 설계 변경 기록에 정리되어 있고 계약 문서의 관련 항목도 함께
수정되었습니다 저장소의 권위 있는 사본은 원격 저장소이며 모든 커밋은 그곳에 반영되어야 합니다""")*60

def write(n,ws): open(os.path.join(OUT,n),"w",encoding="utf-8").write(" ".join(ws)+"\n")

# ratio sweep: r = fraction of words that are ENGLISH, rest Korean
for n in (200, 2000):
    for pct in range(0, 101, 10):
        out=[]
        for i in range(n):
            out.append(en[i % len(en)] if (i*100//n) % 100 < 0 or random.random()*100 < pct else KO[i % len(KO)])
        write(f"mix{pct:03d}-{n}.txt", out)

# adversarial singles
write("ja-200.txt", words("""この変更は文書全体に残っていた古い説明を整理します スキャナが外部コンテンツの中で
生テキスト状態に入る問題を修正し その結果ブラウザが実際に解釈する方法と同じように動作するようになりました
検証層は翻訳されたペイロードにも元と同じ入れ子の深さ制限を適用します""")*8)
write("zh-200.txt", words("""此更改整理了整个文档中遗留的过时说明 修复了扫描器在外部内容中进入原始文本状态的问题
因此其行为与浏览器实际解析的方式一致 验证层现在对翻译后的负载也应用与源文件相同的嵌套深度限制""")*10)
write("ko-romanized-200.txt", words("""i byeongyeongeun munseo jeonbane geolchyeo nama itdeon oraedoen
seolmyeongeul jeongnihamnida seukaeneoga oebu kontencheu aneseo wonsi tekseuteu sangtaero jinipadeon
munjereul gochyeotgo geu gyeolgwa beuraujeoga siljero haeseokhaneun bangsikgwa donghilhage""")*10)
en200=en[:180]
write("en-with-ko-quote-200.txt", en200[:150] + words("사용자는 결과를 확인한 다음 다음 단계를 결정하면 됩니다") + en200[150:])
print("wrote", len(os.listdir(OUT)), "files")
