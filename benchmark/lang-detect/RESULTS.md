# Which detector backs the language gate — measured 2026-09-02

Decision input for ticket `eb1d89`. Reproduce with:

```bash
cargo run --release -- corpus        # committed, synthetic
python3 scripts/build-corpus-real.py # needs a resp-translator checkout
cargo run --release -- corpus-real
```

## Verdict: `whichlang`

| | real fixtures | flip point | throughput | licence |
|---|---|---|---|---|
| **`whichlang`** | **14/14** | **70 % en** | 0.042 ms | MIT |
| `script` (no dependency) | 14/14 | 60 % en | 0.004 ms | — |
| `lingua` (ko+en) | 14/14 | **30–40 % en** | 0.179 ms | Apache-2.0 |
| `whatlang` | 10/14 | unusable | 0.008 ms | MIT |

"Flip point" is the proportion of English words at which a detector stops
calling a Korean answer Korean. Later is better here: an agent explaining code
in Korean emits a lot of English identifiers, and that answer is still Korean.

## What the two rounds cost, and why both were needed

**Round 1 decided nothing.** On the authored corpus every candidate scored
15/15 — Hangul against Latin is trivially separable, so the benchmark had no
discriminating power. Reporting that as "all equivalent, pick the cheapest"
would have been wrong twice over.

**Round 2 reversed the ranking.** `lingua` was the initial recommendation, on
its documented strength with short text. Against real agent output it flips
*earliest* of the three libraries — at 30–40 % English — which is the expensive
direction: it would call a Korean answer non-Korean, spend a provider call, and
emit ko→ko. Its advantage did not survive contact with the domain.

**`whatlang` is out on evidence, not preference.** Two of its four misses are
the costly kind — genuine Korean called not-Korean at 50 words of prose and 200
words raw. The other two are it returning `Unknown` for *pure English* at 200
and 1000 words, which is safe for a gate but means `is_reliable()` is noise on
technical English. Across the sweep it abstains almost everywhere.

## The control is the uncomfortable part

A Hangul codepoint test with no dependency scores the same 14/14 and is ten
times faster than the winner. `whichlang` earns its place on two things the
control cannot do: it names Japanese and Chinese as not-Korean where the
control abstains, and it generalises past the two scripts this gate happens to
need today. **Design that follows: run the script test first, the library
second** — Hangul is decisive when present, so the library only sees the cases
the free test cannot settle.

## A finding that cuts against the ticket's premise

Stripping code spans and tables — what `transync_html::extract` would hand over
— raises the real fixture's Hangul share from 64.6 % to 82.3 %, but **changed no
verdict** for any of the three sound detectors. Extraction buys *margin*, not
correctness: it moves the operating point from 35 % Latin to 18 %, further from
every flip point. That is real robustness against a future identifier-heavier
document, but it is **not** an accuracy argument for routing this gate through
transync. If the facade's translatable-text function is justified, it is
justified as a capability consumers asked for — not by this measurement.

## Honest limits

- The Korean half of `corpus/` is **authored for this benchmark**, not sampled.
  Because every candidate sees identical input the *ranking* holds, but the
  absolute flip points come from `corpus-real/`, not from it.
- `corpus-real/` derives from `resp-translator`'s fixtures and is gitignored —
  another repository's material is not this one's to vendor.
- Romanized Korean (`annyeonghaseyo`) fails on all four, so it separates
  nothing and no candidate should be credited for it.
- `mcp-sample.ko.txt` in that fixture set is 0 % Hangul — entirely English
  despite its name. Flagged to the owner; not used as Korean input here.
