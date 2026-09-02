#!/usr/bin/env python3
"""Build `corpus-real/` from resp-translator's own agent-response fixtures.

The synthetic corpus in `corpus/` is authored and committed; this one is
derived from another repository's material, so it is gitignored and has to be
regenerated locally. Point the two paths below at your checkout.

    python3 scripts/build-corpus-real.py [RESP_TRANSLATOR_ROOT]

Why it matters: round 1 of this benchmark ran on the authored corpus and scored
every candidate 15/15, deciding nothing. Real agent output — Korean prose dense
with English identifiers, markdown tables and code spans — is what separated
them, and it reversed the ranking.
"""
import os, random, re, sys

ROOT = sys.argv[1] if len(sys.argv) > 1 else "/Volumes/Common/QJoon/resp-translator"
KO_F = os.path.join(ROOT, "tests/fixtures/testcase.ko.txt")
EN_F = os.path.join(ROOT, "mcp-sample.txt")
OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "corpus-real")

random.seed(23)
os.makedirs(OUT, exist_ok=True)

def strip_code(t):
    """Approximate what `transync_html::extract` hands over: prose only."""
    t = re.sub(r"```.*?```", "", t, flags=re.S)
    t = re.sub(r"`[^`\n]*`", "", t)
    t = re.sub(r"^\s*\|.*$", "", t, flags=re.M)
    t = re.sub(r"https?://\S+", " ", t)
    return t

def words(t):
    return [w for w in re.split(r"\s+", t) if w]

for p in (KO_F, EN_F):
    if not os.path.exists(p):
        sys.exit(f"missing fixture: {p}\npass your resp-translator root as argv[1]")

ko_raw = open(KO_F, encoding="utf-8", errors="replace").read()
en_raw = open(EN_F, encoding="utf-8", errors="replace").read()
ko_prose, en_prose = strip_code(ko_raw), strip_code(en_raw)

def hangul_share(t):
    h = sum(1 for c in t if "가" <= c <= "힯")
    l = sum(1 for c in t if c.isascii() and c.isalpha())
    return 100 * h / max(1, h + l)

print("korean fixture hangul share  raw %.1f%%  prose-only %.1f%%"
      % (hangul_share(ko_raw), hangul_share(ko_prose)))

kw_raw, kw_pr, ew = words(ko_raw), words(ko_prose), words(en_prose)

def w(name, ws):
    open(os.path.join(OUT, name), "w", encoding="utf-8").write(" ".join(ws) + "\n")

for n in (20, 50, 200, 1000):
    w(f"real-ko-raw-{n}.txt", (kw_raw * 20)[:n])
    w(f"real-ko-prose-{n}.txt", (kw_pr * 20)[:n])
    w(f"real-en-{n}.txt", ew[:n])
w("real-ko-full.txt", kw_raw)
w("real-ko-full-prose.txt", kw_pr)

# The sweep that decided it: real Korean prose diluted with real English.
for n in (200, 2000):
    for pct in range(0, 101, 10):
        out = [ew[i % len(ew)] if random.random() * 100 < pct else kw_pr[i % len(kw_pr)]
               for i in range(n)]
        w(f"rmix{pct:03d}-{n}.txt", out)

print("wrote", len(os.listdir(OUT)), "files to", OUT)
