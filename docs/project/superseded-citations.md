---
type: reference
title: Superseded citations — names and claims in records that no longer resolve
description: Records are write-once (DCR-0054), so a dated record that cites a symbol since renamed, or states a count since corrected, cannot be repaired in place. This living register is the forwarding address. It maps each stale citation to what it is now and says why the name changed, so a reader following a record's citation is not left at a dead end.
tags: [reference, project-control, DCR-0054]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-13T00:00:00Z
status: stable
---

# Superseded citations

A record — a DCR, a released `CHANGELOG` section, a spec, an archived issue — is
**write-once** (DCR-0054). Its words are the words it was written with, and
nothing is appended to it, so when a symbol it cites gets renamed or a count it
states turns out wrong, the record cannot be repaired.

This is where the repair goes instead. It is a **living** document: each row says
what a stale citation resolves to today. Follow a record's citation here when it
does not resolve in the tree.

This register is **seeded, not exhaustive**. It was started on 2026-09-13 from
the forwarding table that the `html-wave0` implementation plan carried, at the
point that plan was retired. Add a row whenever a rename or a correction strands
a citation in a record.

## Renamed symbols

Wave 1 of the HTML→HTML feature (`d146a53`) changed the rule that wave 0's
Task 3 had written, so three names wave 0 introduced no longer exist.

| cited as | resolves to | why the name had to change |
|---|---|---|
| `void_and_self_closing_elements_mint_no_extent` | `void_elements_mint_no_extent_and_a_flagged_non_void_one_does` | a flagged non-void element **does** mint an extent now |
| `a_slash_outside_any_attribute_value_still_self_closes` | `a_slash_outside_any_attribute_value_sets_the_flag` | the scanner still *sets* the flag; setting it no longer means the element closes itself |
| the guard `!is_void(name) && (!self_closing \|\| is_raw_text(name))` | `!is_void(name) && !(*self_closing && honours_flag)` in `crates/transync-html/src/lib.rs` | HTML honours the flag in exactly two places — inside foreign content, and on the `<svg>`/`<math>` start tags that enter it |

**`DCR-0016` still cites the second of these** and cannot be corrected, which is
the reason this register exists rather than a note on the record. The reasoning
behind the rename is in DCR-0032's 2026-08-23 amendment; see also
`docs/architecture/transync-html-divergences.md`, divergence 2.

## Corrected claims

**`c5ff3df`'s commit message says the wave-0 + wave-1 arc "removed exactly two"
`fn` names. The count is three.** The third is
`void_and_self_closing_elements_mint_no_extent`, which was created inside wave 0
and renamed in wave 1 — so it appears in neither endpoint of the single
`a96589b..HEAD` intersection that produced the claim, and a single pair cannot
see it. A four-baseline run (`59ce8df`, `6fa4e88`, `a96589b`, `1299280`) returns
2 / 2 / 2 / **3**.

A commit message cannot be corrected without rewriting history, so the
correction lives here.

**The generalisation, which is the part worth keeping:** a symbol intersection is
complete only for names that existed at the baseline you chose. A name born and
renamed between two commits is invisible to a comparison of their endpoints, so
a sweep needs several baselines or it is a sample wearing a proof's clothes. The
instrument was sound; running it once was not.

## Retired documents

The thirteen executed implementation plans under `docs/superpowers/plans/` were
retired on 2026-09-13. They were 1,505,956 bytes — 27% of all tracked English
Markdown — and between 63% and 89% of each was a frozen copy of code that had
already shipped.

**Nine records cite them by path and cannot be corrected**, because a record is
write-once:

| record | cited plan |
|---|---|
| `DCR-0020` | `2026-08-05-track-c-wasm-render-demo.md` |
| `DCR-0034` | `2026-08-20-html-wave2-ir-split.md` |
| `DCR-0035` | `2026-08-20-html-wave3-intake-round-trip.md` |
| `DCR-0036` | `2026-08-20-html-wave4-layer6-twin.md` |
| `DCR-0037` | `2026-08-20-html-wave5-units-context-prompt.md` |
| `DCR-0038` | `2026-08-20-html-wave6-panes-wire-cli.md` |
| `DCR-0040` | `2026-08-20-html-wave*.md` (as a group) |
| `archive/DCR-0019` | `2026-08-05-oi0008-0033-internal-quality.md` |
| `archive/DCR-0017`, `archive/DCR-0018` | their waves' plans, by description |

**What to read instead.** Each plan's own DCR is the authoritative record of what
that wave changed and why; the `[0.4.0]` and `[0.5.0]` sections of
`CHANGELOG-archive.md` and `CHANGELOG.md` record what shipped. The plans held the
task breakdown and the pre-implementation code snapshots, neither of which is
current truth about anything.

The design **specs** beside them under `docs/superpowers/specs/` are **not**
retired — they hold live design content, and two of them are cited from Rust
source.
