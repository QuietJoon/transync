//! Inline-protection layer: link/image destination identity,
//! (policy-gated) inline code-span identity, and always-on raw inline
//! HTML tag identity. EXT-2026-07 P1-5; amends ADR-0012 — inline TEXT
//! stays LLM-owned, destinations, pledged code spans and raw inline tags
//! are operational metadata.
//!
//! The raw-inline-HTML tag guard (spec 2026-08-03 §4.3) is **not**
//! policy-gated: every `<kbd>`, `</kbd>`, `<br>` token must reappear
//! byte-for-byte and in source order, because a model has no legitimate
//! reason to alter operational markup. It is therefore hoisted above the
//! destination/code-span gates, which is why the two payload parses now
//! run even for a fully permissive profile (accepted cost, spec §4.3).
//! `InputMode::HtmlSegments` units skip the whole layer: their payload is a
//! JSON segment array, whose structure the splice check owns.
//!
//! The compare is post-translation: the source payload and the
//! translated payload are each walked with the canonical
//! [`crate::markdown::comrak_options`] parse, so entity/backslash-escape
//! resolution and angle-bracket stripping cancel out symmetrically. A
//! mismatch is a retryable [`super::ValidationLayer::Inline`] rejection —
//! there is no repair pass (the pipeline never rewrites provider output;
//! invariant 6, ADR-0009).
//!
//! Autolinks (`extension.autolink = true`) contribute to the same
//! ordered destination sequence as explicit links: altering a bare URL
//! rejects, but converting an autolink to `[text](same-url)` (or vice
//! versa) passes — the destination sequence is identical and the form is
//! presentation, not sync currency. Known edge: translated CJK
//! punctuation placed flush against a bare URL can shift the GFM autolink
//! boundary and change the parsed destination; that is a genuine
//! render-visible change, so it rejects — retry usually resolves it
//! (providers insert a space) and fallback is safe.
//!
//! Reference-style links/images (`[text][ref]`, `![alt][ref]`, collapsed
//! `[ref][]`, and shortcut `[ref]`) resolve against the document's own
//! definitions ([`crate::refmap::ReferenceMap`], built once per run by asking
//! comrak), wherever those definitions sit — an inter-block gap, inside a
//! blockquote or a list item, or ahead of a paragraph's text. Each payload is
//! parsed alone, and a label it does not define is handed to that map
//! through comrak's `broken_link_callback`; nothing is appended to the
//! payload. Both sides resolve against the same map under one expansion
//! budget, so the compare stays symmetric: a reference whose label the model
//! preserves resolves identically and passes, while a dropped reference or a
//! translated label breaks resolution on the translated side only — a
//! destination-count mismatch that rejects (retryable
//! [`super::ValidationLayer::Inline`]). A genuinely undefined reference (no
//! definition anywhere) stays literal text on both sides — a symmetric no-op.
//!
//! A definition a payload carries **itself** is text the provider can change
//! without adding or removing an inline node, so it gets its own comparison
//! ([`crate::refmap::definition_change`]): the definitions on each side must
//! agree label for label and destination for destination, which makes a
//! rewritten, deleted or injected definition the same retryable rejection.
//! Gated with destinations, by `preserve_urls`. The full-document gate
//! compares every block's resolved destinations once more after
//! regeneration (`validate::full_reparse`, scan 4), under the same gate.
//!
//! TRACE: R0013-0003
//! TRACE: EXT-2026-07 P1-5
//! TRACE: ADR-0012 (amendment §4)

use crate::id::BlockKind;
use crate::llm::{InputMode, TranslationUnit, UnitResult};
use crate::profile::ProfileConstraints;
use crate::refmap::ReferenceMap;

/// Whether a destination came from a link or an image node — part of the
/// compared identity so a link↔image flip with the same URL still rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DestKind {
    Link,
    Image,
}

/// Inline inventory extracted from one payload by a comrak walk.
#[derive(Debug, Default, PartialEq)]
struct InlineInventory {
    /// Link/image destinations in pre-order document order.
    destinations: Vec<(DestKind, String)>,
    /// Inline code-span literals (compared as an unordered multiset).
    code_spans: Vec<String>,
    /// Raw inline HTML tag tokens (`<kbd>`, `</kbd>`, `<br>`, …) in
    /// pre-order document order. Compared verbatim and in order, always
    /// (spec 2026-08-03 §4.3) — no policy can disable it.
    html_tokens: Vec<String>,
}

/// Compare inline inventories of the source vs translated payload.
///
/// Returns `Ok(())` when every enforced inventory matches. On mismatch,
/// returns a diagnostic string naming the divergence (values are
/// [`clip`]-ed).
///
/// Always enforced, before any gate (spec 2026-08-03 §4.3):
/// - ordered, verbatim raw inline HTML tag identity.
///
/// Policy gates (see the ADR-0012 amendment and the design's policy
/// matrix):
/// - destination identity is enforced unless `preserve_urls == Some(false)`;
/// - inline code-span identity is enforced only when
///   `preserve_code_identifiers == Some(true)`.
///
/// `BlockKind::CodeBlock` units and `InputMode::HtmlSegments` units return
/// early: fence bodies carry no inline nodes, and an html unit's payload is
/// a JSON segment array validated by the splice check instead.
///
/// `refs` is the document's effective link-reference definitions
/// ([`ReferenceMap`]); both payloads resolve a label they do not define
/// against it, under one shared expansion budget. Pass
/// [`ReferenceMap::empty`] when the document defines none. Under destination
/// enforcement, the definitions each payload carries itself must also agree
/// ([`crate::refmap::definition_change`]).
///
/// TRACE: EXT-2026-07 P1-5
/// TRACE: R0013-0003
pub fn check_inline(
    policy: &ProfileConstraints,
    unit: &TranslationUnit,
    result: &UnitResult,
    refs: &ReferenceMap,
) -> Result<(), String> {
    // Fences carry no inline nodes; html payloads are JSON (the splice check
    // owns their structure). Skip both parses. ti 490d97 wave 2: the html half
    // is keyed on the MODE — an HTML document's `<p>` is a Paragraph whose
    // payload is a segment array, and running the inline inventory over JSON
    // would compare bracket counts and call them links.
    if matches!(unit.block_kind, BlockKind::CodeBlock { .. })
        || matches!(unit.input_mode, InputMode::HtmlSegments)
    {
        return Ok(());
    }

    let enforce_dest = destinations_enforced(policy);
    let enforce_code = policy.preserve_code_identifiers == Some(true);

    // One budget for both walks, so a reference sequence the two sides share
    // stops resolving at the same point on both (`refmap::expansion_budget`).
    let budget =
        crate::refmap::expansion_budget(&[&unit.source_payload, &result.translated_payload]);
    let source = inline_inventory(&unit.source_payload, refs, budget);
    let translated = inline_inventory(&result.translated_payload, refs, budget);

    // Spec §4.3: always-on ordered raw-inline-HTML tag identity — hoisted
    // ABOVE the policy gates so no profile can disable it. A model has no
    // legitimate reason to alter a tag.
    let (s, t) = (&source.html_tokens, &translated.html_tokens);
    if s.len() != t.len() {
        return Err(format!(
            "raw inline HTML tag count changed: source has {}, translated has {}",
            s.len(),
            t.len()
        ));
    }
    for (i, (a, b)) in s.iter().zip(t.iter()).enumerate() {
        if a != b {
            return Err(format!(
                "raw inline HTML tag {i} changed: source {}, translated {}",
                clip(a),
                clip(b)
            ));
        }
    }

    if !enforce_dest && !enforce_code {
        return Ok(());
    }

    if enforce_dest {
        let (s, t) = (&source.destinations, &translated.destinations);
        if s.len() != t.len() {
            return Err(format!(
                "inline destination count changed: source has {}, translated has {}",
                s.len(),
                t.len()
            ));
        }
        for (i, ((skind, surl), (tkind, turl))) in s.iter().zip(t.iter()).enumerate() {
            if skind != tkind || surl != turl {
                return Err(format!(
                    "inline destination {i} changed: source {skind:?} {}, translated {tkind:?} {}",
                    clip(surl),
                    clip(turl)
                ));
            }
        }
        // R0013-0003 (c): a definition the payload carries is a destination
        // no inline node of this payload shows — the links that use it are in
        // other blocks. A rewrite, a deletion or an injection is this unit's
        // fault and gets this unit's retry.
        if let Some(change) =
            crate::refmap::definition_change(&unit.source_payload, &result.translated_payload)
        {
            return Err(change);
        }
    }

    if enforce_code {
        let (s, t) = (&source.code_spans, &translated.code_spans);
        if s.len() != t.len() {
            return Err(format!(
                "inline code-span count changed: source has {}, translated has {}",
                s.len(),
                t.len()
            ));
        }
        // Unordered multiset: word-order movement of inline identifiers is
        // routine in translation, so match each source span against a
        // consumed copy of the translated spans and report the first source
        // span with no counterpart.
        let mut remaining: Vec<&String> = t.iter().collect();
        for span in s {
            match remaining.iter().position(|x| *x == span) {
                Some(pos) => {
                    remaining.swap_remove(pos);
                }
                None => {
                    return Err(format!(
                        "inline code span altered: {} missing from translation",
                        clip(span)
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Whether `policy` enforces link/image destination identity: `preserve_urls`
/// unset or `true`. THE predicate — the inline layer and the full-document
/// gate's destination scan (`validate::full_reparse`, scan 4) both read it, so
/// the two cannot disagree about which runs protect destinations.
pub(crate) fn destinations_enforced(policy: &ProfileConstraints) -> bool {
    policy.preserve_urls != Some(false)
}

/// Walk `payload` under the canonical GFM options and collect its inline
/// inventory: link/image destinations in document order, inline code-span
/// literals, and raw inline HTML tag tokens in document order.
///
/// A reference the payload does not define resolves through `refs`, charged
/// against `budget` ([`ReferenceMap::parse`]); definitions produce no inline
/// nodes, so they never add inventory entries of their own.
fn inline_inventory(payload: &str, refs: &ReferenceMap, budget: usize) -> InlineInventory {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    // Past the ceiling there is no inventory to take: return an empty one
    // rather than build a tree. The comparison against the source inventory
    // then fails and the unit is rejected — which is the right outcome, and
    // the one `validate_unit`'s door has already produced with a better
    // reason before this is reachable.
    let Ok(root) = refs.parse(&arena, payload, budget) else {
        return InlineInventory::default();
    };

    let mut inv = InlineInventory::default();
    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Link(l) => inv.destinations.push((DestKind::Link, l.url.clone())),
            NodeValue::Image(l) => inv.destinations.push((DestKind::Image, l.url.clone())),
            NodeValue::Code(c) => inv.code_spans.push(c.literal.clone()),
            NodeValue::HtmlInline(literal) => inv.html_tokens.push(literal.clone()),
            _ => {}
        }
    }
    inv
}

/// Truncate a value for a diagnostic so a `data:` URI (or any long
/// destination) can't produce a megabyte-long `rejection_reason`. Cuts on
/// a char boundary and appends `…` only when truncation happened.
pub(crate) fn clip(s: &str) -> String {
    const MAX: usize = 120;
    if s.chars().count() <= MAX {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, InputMode, OutputKind, TranslationUnit, UnitResult,
    };

    fn unit(kind: BlockKind, source_payload: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId("x-0001".to_string()),
            block_kind: kind,
            input_mode: InputMode::TextFragment,
            source_payload: source_payload.to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn unit_result(payload: &str) -> UnitResult {
        UnitResult {
            unit_id: BlockId("x-0001".to_string()),
            output_kind: OutputKind::Translated,
            translated_payload: payload.to_string(),
            warnings: Vec::new(),
        }
    }

    fn policy(urls: Option<bool>, code: Option<bool>) -> ProfileConstraints {
        ProfileConstraints {
            preserve_urls: urls,
            preserve_code_identifiers: code,
            default_table_strategy: None,
        }
    }

    // (1) review-required tampered-destination case.
    #[test]
    fn tampered_link_destination_is_rejected() {
        let u = unit(
            BlockKind::Paragraph,
            "See [the docs](https://example.com/a).",
        );
        let r = unit_result("[문서](https://evil.example/x)를 보라.");
        let err = check_inline(&policy(Some(true), None), &u, &r, &none()).unwrap_err();
        assert!(err.contains("destination 0"), "should name index 0: {err}");
        assert!(
            err.contains("https://example.com/a"),
            "names source url: {err}"
        );
        assert!(
            err.contains("https://evil.example/x"),
            "names translated url: {err}"
        );
    }

    #[test]
    fn translated_link_text_same_destination_passes() {
        let u = unit(
            BlockKind::Paragraph,
            "See [the docs](https://example.com/a).",
        );
        let r = unit_result("[문서](https://example.com/a)를 보라.");
        assert!(check_inline(&policy(Some(true), None), &u, &r, &none()).is_ok());
    }

    #[test]
    fn added_link_is_rejected() {
        let u = unit(BlockKind::Paragraph, "Plain sentence, no links.");
        let r = unit_result("[링크](https://example.com/a) 추가된 문장.");
        let err = check_inline(&policy(Some(true), None), &u, &r, &none()).unwrap_err();
        assert!(err.contains("count changed"), "{err}");
    }

    #[test]
    fn dropped_link_is_rejected() {
        let u = unit(
            BlockKind::Paragraph,
            "See [the docs](https://example.com/a).",
        );
        let r = unit_result("문서를 보라.");
        let err = check_inline(&policy(Some(true), None), &u, &r, &none()).unwrap_err();
        assert!(err.contains("count changed"), "{err}");
    }

    // (4) review-required rewrite-allowed case.
    #[test]
    fn preserve_urls_false_permits_rewritten_destination() {
        let u = unit(
            BlockKind::Paragraph,
            "See [the docs](https://example.com/a).",
        );
        let r = unit_result("[문서](https://evil.example/x)를 보라.");
        assert!(check_inline(&policy(Some(false), None), &u, &r, &none()).is_ok());
    }

    #[test]
    fn unset_preserve_urls_still_enforces() {
        // Policy-matrix `None` cell: unset defaults to enforce.
        let u = unit(
            BlockKind::Paragraph,
            "See [the docs](https://example.com/a).",
        );
        let r = unit_result("[문서](https://evil.example/x)를 보라.");
        assert!(check_inline(&ProfileConstraints::default(), &u, &r, &none()).is_err());
    }

    #[test]
    fn autolink_destination_change_is_rejected() {
        let u = unit(BlockKind::Paragraph, "Visit https://example.com/a today.");
        let r = unit_result("오늘 https://evil.example/x 방문하세요.");
        assert!(check_inline(&policy(Some(true), None), &u, &r, &none()).is_err());
    }

    #[test]
    fn autolink_to_explicit_link_same_destination_passes() {
        // Form change with an identical destination is presentation only.
        let u = unit(BlockKind::Paragraph, "Visit https://example.com/a today.");
        let r = unit_result("오늘 [여기](https://example.com/a) 방문하세요.");
        assert!(check_inline(&policy(Some(true), None), &u, &r, &none()).is_ok());
    }

    #[test]
    fn image_src_change_is_rejected() {
        let u = unit(BlockKind::Paragraph, "Logo ![alt](img/a.png) here.");
        let r = unit_result("로고 ![대체](img/evil.png) 여기.");
        let err = check_inline(&policy(Some(true), None), &u, &r, &none()).unwrap_err();
        assert!(err.contains("Image"), "kind-tagged as Image: {err}");
    }

    #[test]
    fn image_alt_translation_passes() {
        let u = unit(BlockKind::Paragraph, "Logo ![alt text](img/a.png) here.");
        let r = unit_result("로고 ![대체 텍스트](img/a.png) 여기.");
        assert!(check_inline(&policy(Some(true), None), &u, &r, &none()).is_ok());
    }

    #[test]
    fn link_kind_flip_to_image_is_rejected() {
        // Same URL, but Link → Image: the kind tag catches it.
        let u = unit(BlockKind::Paragraph, "See [x](u/a) now.");
        let r = unit_result("![x](u/a) 보라.");
        let err = check_inline(&policy(Some(true), None), &u, &r, &none()).unwrap_err();
        assert!(err.contains("destination 0"), "{err}");
    }

    #[test]
    fn link_title_translation_passes() {
        // Titles are translatable — not compared.
        let u = unit(
            BlockKind::Paragraph,
            "See [x](https://example.com/a \"old title\").",
        );
        let r = unit_result("[엑스](https://example.com/a \"새 제목\")를 보라.");
        assert!(check_inline(&policy(Some(true), None), &u, &r, &none()).is_ok());
    }

    #[test]
    fn code_span_alteration_is_rejected_when_pledged() {
        let u = unit(BlockKind::Paragraph, "Call `foo()` then `bar()`.");
        let r = unit_result("`foo()` 다음 `baz()` 호출.");
        let err = check_inline(&policy(None, Some(true)), &u, &r, &none()).unwrap_err();
        assert!(err.contains("code span altered"), "{err}");
    }

    #[test]
    fn code_span_reorder_passes_as_multiset() {
        let u = unit(BlockKind::Paragraph, "Call `foo()` then `bar()`.");
        let r = unit_result("`bar()` 다음 `foo()` 호출.");
        assert!(check_inline(&policy(None, Some(true)), &u, &r, &none()).is_ok());
    }

    #[test]
    fn code_span_change_is_advisory_when_unset() {
        // preserve_code_identifiers unset → advisory only, no code check.
        let u = unit(BlockKind::Paragraph, "Call `foo()` then `bar()`.");
        let r = unit_result("`foo()` 다음 `baz()` 호출.");
        assert!(check_inline(&policy(None, None), &u, &r, &none()).is_ok());
    }

    #[test]
    fn table_cell_link_tampering_is_rejected() {
        // Proves the walk reaches cell inlines in a Table-kind payload.
        let src = "| Site | URL |\n|------|-----|\n| Ex | [link](https://example.com/a) |\n";
        let tam = "| 사이트 | URL |\n|------|-----|\n| 예 | [링크](https://evil.example/x) |\n";
        let u = unit(BlockKind::Table, src);
        let r = unit_result(tam);
        let err = check_inline(&policy(Some(true), None), &u, &r, &none()).unwrap_err();
        assert!(err.contains("destination 0"), "{err}");
    }

    #[test]
    fn code_block_unit_is_skipped() {
        // A CodeBlock body whose comment holds a URL passes even under
        // enforcement — fence content has no inline nodes.
        let body = "```sh\ncurl https://example.com/a\n```\n";
        let tampered = "```sh\ncurl https://evil.example/x\n```\n";
        let u = unit(
            BlockKind::CodeBlock {
                info: Some("sh".to_string()),
                fenced: true,
            },
            body,
        );
        let r = unit_result(tampered);
        assert!(check_inline(&policy(Some(true), Some(true)), &u, &r, &none()).is_ok());
    }

    // Design D2 §B4 / C10, R0013-0003: reference-style links resolve from the
    // document's own definitions, closing the ADR-0012-amendment §4 gap.
    const REF_POOL: &str = "[ref]: https://example.com/r\n";

    #[test]
    fn reference_label_preserved_text_translated_passes() {
        // (a) The reference LABEL is preserved and only the link text is
        // translated → both sides resolve `[ref]` to the same destination.
        let u = unit(BlockKind::Paragraph, "See [the docs][ref] for more.");
        let r = unit_result("자세한 내용은 [문서][ref].");
        assert!(check_inline(&policy(Some(true), None), &u, &r, &doc_refs(REF_POOL)).is_ok());
    }

    #[test]
    fn reference_label_translation_is_rejected() {
        // (b) The model translated the reference LABEL (`ref` → `참조`), which
        // is undefined on the translated side → resolution breaks there only
        // → destination-count mismatch. Previously a silent v1 gap.
        let u = unit(BlockKind::Paragraph, "See [the docs][ref] for more.");
        let r = unit_result("자세한 내용은 [문서][참조].");
        let err = check_inline(&policy(Some(true), None), &u, &r, &doc_refs(REF_POOL)).unwrap_err();
        assert!(
            err.contains("count changed"),
            "label break is a count mismatch: {err}"
        );
    }

    #[test]
    fn added_reference_use_is_rejected() {
        // (c) Both sides resolve from the SAME pool, so a use-site cannot be
        // silently redirected via refs; instead an ADDED reference use is the
        // observable tamper → count mismatch.
        let u = unit(BlockKind::Paragraph, "See [the docs][ref].");
        let r = unit_result("[문서][ref] 그리고 [추가][ref] 보라.");
        let err = check_inline(&policy(Some(true), None), &u, &r, &doc_refs(REF_POOL)).unwrap_err();
        assert!(err.contains("count changed"), "added reference use: {err}");
    }

    #[test]
    fn image_reference_is_enforced_as_image_kind() {
        // (d) `![alt][ref]` resolves as an Image destination; flipping it to
        // a link reference `[text][ref]` (same label, same pooled URL) keeps
        // the count but changes the DestKind → rejected, kind-tagged Image.
        let pool = "[ref]: https://example.com/logo.png\n";
        let u = unit(BlockKind::Paragraph, "Logo ![logo][ref] here.");
        let r = unit_result("로고 [로고][ref] 여기.");
        let err = check_inline(&policy(Some(true), None), &u, &r, &doc_refs(pool)).unwrap_err();
        assert!(
            err.contains("Image"),
            "image reference tagged as Image: {err}"
        );
    }

    #[test]
    fn no_definitions_keeps_symmetric_pass_for_undefined_reference() {
        // (e) With no definitions anywhere, `[text][ref]` stays
        // literal on both sides — the pre-refmap symmetric no-op is preserved.
        let u = unit(BlockKind::Paragraph, "See [the docs][ref] for more.");
        let r = unit_result("자세한 내용은 [문서][참조].");
        assert!(check_inline(&policy(Some(true), None), &u, &r, &none()).is_ok());
    }

    // Spec §4.3: always-on ordered raw-inline-HTML tag identity.
    #[test]
    fn dropped_inline_tag_is_rejected() {
        let u = unit(BlockKind::Paragraph, "press <kbd>Ctrl</kbd> now");
        let err = check_inline(
            &policy(None, None),
            &u,
            &unit_result("press Ctrl now"),
            &none(),
        )
        .unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn mangled_inline_tag_is_rejected() {
        let u = unit(BlockKind::Paragraph, "a <b>bold</b> word");
        let err = check_inline(
            &policy(None, None),
            &u,
            &unit_result("a <strong>bold</strong> word"),
            &none(),
        )
        .unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn reordered_inline_tags_are_rejected() {
        let u = unit(BlockKind::Paragraph, "<sup>a</sup> then <sub>b</sub>");
        let err = check_inline(
            &policy(None, None),
            &u,
            &unit_result("<sub>b</sub> then <sup>a</sup>"),
            &none(),
        )
        .unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn tag_guard_survives_permissive_policy() {
        // The old early-return (preserve_urls=false, no code pledge) must
        // NOT disable the guard — hoisted above the policy gates.
        let u = unit(BlockKind::Paragraph, "x <br> y");
        let err =
            check_inline(&policy(Some(false), None), &u, &unit_result("x y"), &none()).unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn text_only_change_around_tags_passes() {
        let u = unit(BlockKind::Paragraph, "press <kbd>Ctrl</kbd> now");
        assert!(
            check_inline(
                &policy(None, None),
                &u,
                &unit_result("지금 <kbd>Ctrl</kbd> 누르세요"),
                &none()
            )
            .is_ok()
        );
    }

    #[test]
    fn html_units_skip_the_inline_layer() {
        // The payload carries a raw tag on the source side and none on the
        // translated side, so the always-on tag-identity guard WOULD reject it
        // if this layer ran. That is what makes the `is_ok()` below a
        // statement about the skip rather than about two payloads that happen
        // to contain nothing the layer inspects.
        let mut u = unit(BlockKind::Html, "[\"press <kbd>Ctrl</kbd>\"]");
        u.input_mode = InputMode::HtmlSegments;
        assert!(
            check_inline(
                &policy(None, None),
                &u,
                &unit_result("[\"누르세요\"]"),
                &none()
            )
            .is_ok(),
            "an html-segments unit must not reach the inline layer at all",
        );

        // The contrast: the same two payloads under a Markdown mode DO reject,
        // which is the layer this skip is bypassing.
        let md = unit(BlockKind::Paragraph, "press <kbd>Ctrl</kbd>");
        assert!(
            check_inline(&policy(None, None), &md, &unit_result("누르세요"), &none()).is_err(),
            "the guard the skip bypasses must be live, or the skip proves nothing",
        );
    }

    // Spec §4.3 known false-reject, pinned as accepted behavior: a tag
    // legally moved to start a line reclassifies the remainder as an HTML
    // block on the translated side only — the tokens vanish and the unit
    // rejects (verbatim retry usually recovers in the pipeline).
    //
    // Only a CommonMark *type-6* tag (`<div>`, `<p>`, …) can do this, since
    // type 6 is the one HTML-block start condition that may interrupt a
    // paragraph; see the sibling test for the type-7 boundary.
    #[test]
    fn line_initial_inline_tag_reclassification_false_rejects() {
        let u = unit(BlockKind::Paragraph, "text <div>bold</div> tail");
        let moved = "text tail\n<div>bold</div>";
        let err = check_inline(&policy(None, None), &u, &unit_result(moved), &none()).unwrap_err();
        assert!(
            err.contains("raw inline HTML tag count changed"),
            "got: {err}"
        );
    }

    // The complement of the case above, pinning where the false-reject does
    // NOT reach: a type-7 tag cannot start an HTML block that interrupts a
    // paragraph, so the moved line stays a lazy continuation with the same
    // inline tokens and the guard passes.
    #[test]
    fn line_initial_type7_tag_stays_inline_and_passes() {
        let u = unit(BlockKind::Paragraph, "text <b>bold</b> tail");
        let moved = "text tail\n<b>bold</b>";
        assert!(check_inline(&policy(None, None), &u, &unit_result(moved), &none()).is_ok());
    }

    // R0013-0003 replaced the gap-pool append with callback resolution, which
    // appends nothing: a payload is parsed as itself, whatever the document
    // defines. These three keep the properties the OI-0040 prefilter tests
    // pinned, restated for a resolver that has no append to skip.

    #[test]
    fn a_bracket_free_payload_has_the_same_inventory_with_or_without_definitions() {
        // Nothing in a bracket-free payload can name a label, so the
        // document's definitions cannot change what the walk finds.
        let refs = doc_refs(REF_POOL);
        let budget = crate::refmap::expansion_budget(&[]);
        let src = "Plain prose with `code`, a <kbd>tag</kbd> and no brackets.";
        let tgt = "괄호 없는 산문, `code`, <kbd>tag</kbd>.";
        assert_eq!(
            inline_inventory(src, &refs, budget),
            inline_inventory(src, &none(), budget)
        );
        assert_eq!(
            inline_inventory(tgt, &refs, budget),
            inline_inventory(tgt, &none(), budget)
        );
        let u = unit(BlockKind::Paragraph, src);
        assert!(
            check_inline(
                &policy(Some(true), Some(true)),
                &u,
                &unit_result(tgt),
                &refs
            )
            .is_ok()
        );
    }

    #[test]
    fn a_reference_resolves_to_the_documents_destination() {
        let inv = inline_inventory(
            "See [the docs][ref].",
            &doc_refs(REF_POOL),
            crate::refmap::expansion_budget(&[]),
        );
        assert_eq!(
            inv.destinations,
            vec![(DestKind::Link, "https://example.com/r".to_string())],
        );
        let u = unit(BlockKind::Paragraph, "See [the docs][ref] for more.");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("자세한 내용은 [문서][참조]."),
            &doc_refs(REF_POOL),
        )
        .unwrap_err();
        assert!(err.contains("count changed"), "{err}");
    }

    #[test]
    fn added_reference_use_in_bracket_free_source_is_rejected() {
        // The asymmetric pair: the source carries no bracket and the
        // translation ADDS a reference link. Both sides resolve against the
        // same map, so the added link resolves and the count moves. (Under the
        // OI-0040 prefilter this pinned that the skip decision was taken from
        // both payloads; the resolver has no skip to get wrong, and the
        // tamper must still reject.)
        let u = unit(BlockKind::Paragraph, "See the docs.");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("[문서][ref] 참조."),
            &doc_refs(REF_POOL),
        )
        .unwrap_err();
        assert!(
            err.contains("count changed"),
            "an added reference use must still reject: {err}"
        );
    }

    // R0013-0003: definitions the inter-block gaps never see. CommonMark gives
    // a definition document-wide effect wherever it sits — inside a blockquote
    // or a list item (spec example 218), or directly ahead of paragraph text on
    // the paragraph's own lines — and comrak resolves all of them across the
    // whole document. `doc_refs` is the document's definitions exactly as the
    // pipeline hands them to this layer, so each test below is a statement
    // about what a real run sees.
    fn doc_refs(src: &str) -> ReferenceMap {
        ReferenceMap::of(&crate::markdown::parse(src).expect("parses"))
    }

    /// A document that defines nothing.
    fn none() -> ReferenceMap {
        ReferenceMap::empty()
    }

    const BLOCKQUOTE_DEF: &str =
        "> [ref]: https://example.com/\n>\n> Definition holder.\n\nRead [docs][ref].\n";

    #[test]
    fn dropped_reference_to_a_blockquote_definition_is_rejected() {
        let u = unit(BlockKind::Paragraph, "Read [docs][ref].");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("Read docs."),
            &doc_refs(BLOCKQUOTE_DEF),
        )
        .expect_err("the paragraph's one link is gone from the translation");
        assert!(
            err.contains("inline destination count changed: source has 1, translated has 0"),
            "{err}"
        );
    }

    #[test]
    fn dropped_reference_to_a_list_item_definition_is_rejected() {
        let src = "- [ref]: https://example.com/\n- item\n\nRead [docs][ref].\n";
        let u = unit(BlockKind::Paragraph, "Read [docs][ref].");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("Read docs."),
            &doc_refs(src),
        )
        .expect_err("a list item's definition resolves document-wide");
        assert!(err.contains("count changed"), "{err}");
    }

    #[test]
    fn dropped_reference_to_a_paragraph_prefix_definition_is_rejected() {
        // The definition sits on the paragraph's own first line: comrak strips
        // it from the paragraph's content but not from its source range, so
        // the bytes belong to a block and no gap ever holds them.
        let src = "[ref]: https://example.com/\nIntro.\n\nRead [docs][ref].\n";
        let u = unit(BlockKind::Paragraph, "Read [docs][ref].");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("Read docs."),
            &doc_refs(src),
        )
        .expect_err("a paragraph-prefix definition resolves document-wide");
        assert!(err.contains("count changed"), "{err}");
    }

    #[test]
    fn relabelled_reference_to_a_container_definition_is_rejected() {
        let u = unit(BlockKind::Paragraph, "Read [docs][ref].");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("[문서][nope] 읽기."),
            &doc_refs(BLOCKQUOTE_DEF),
        )
        .expect_err("`nope` is defined nowhere, so the link is gone");
        assert!(err.contains("count changed"), "{err}");
    }

    #[test]
    fn preserved_label_of_a_container_definition_passes() {
        // The no-false-reject half: the label kept, the text translated.
        let u = unit(BlockKind::Paragraph, "Read [docs][ref].");
        assert!(
            check_inline(
                &policy(Some(true), None),
                &u,
                &unit_result("[문서][ref] 읽기."),
                &doc_refs(BLOCKQUOTE_DEF),
            )
            .is_ok()
        );
    }

    #[test]
    fn the_first_definition_wins_even_when_it_sits_in_a_container() {
        // The document resolves `ref` to a.example — the blockquote's
        // definition comes first. A translation that spells the link out with
        // the LATER definition's destination therefore changed it.
        let src = "> [ref]: https://a.example/\n\nSee [x][ref].\n\n[ref]: https://b.example/\n";
        let u = unit(BlockKind::Paragraph, "See [x][ref].");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("[엑스](https://b.example/) 보기."),
            &doc_refs(src),
        )
        .expect_err("the source link resolves to a.example, not b.example");
        assert!(err.contains("https://a.example/"), "{err}");
        assert!(err.contains("https://b.example/"), "{err}");
    }

    // R0013-0003 (c): a definition carried inside a payload is text the
    // provider can change. It produces no inline node of its own, so the
    // destination inventory alone cannot see a rewrite, a deletion or an
    // injection — the referencing block is a different unit.

    const QUOTED_DEF: &str = "> [ref]: https://example.com/\n>\n> Definition holder.";

    #[test]
    fn a_rewritten_definition_inside_a_payload_is_rejected() {
        let u = unit(BlockKind::Blockquote, QUOTED_DEF);
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("> [ref]: https://evil.example/\n>\n> 정의 보유자."),
            &doc_refs(BLOCKQUOTE_DEF),
        )
        .expect_err("the definition's destination was rewritten");
        assert!(err.contains("reference definition"), "{err}");
        assert!(err.contains("https://evil.example/"), "{err}");
    }

    #[test]
    fn a_deleted_definition_inside_a_payload_is_rejected() {
        let u = unit(BlockKind::Blockquote, QUOTED_DEF);
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("> 정의 보유자."),
            &doc_refs(BLOCKQUOTE_DEF),
        )
        .expect_err("the definition every `[..][ref]` resolves to is gone");
        assert!(err.contains("reference definition"), "{err}");
    }

    #[test]
    fn an_injected_definition_is_rejected() {
        // A gap-only document: `Intro.` grows a definition that takes over the
        // later gap-defined label, because the first definition wins.
        let src = "Intro.\n\nSee [docs][ref].\n\n[ref]: https://example.com/\n";
        let u = unit(BlockKind::Paragraph, "Intro.");
        let err = check_inline(
            &policy(Some(true), None),
            &u,
            &unit_result("[ref]: https://evil.example/\n소개."),
            &doc_refs(src),
        )
        .expect_err("the translation defines a label its source never did");
        assert!(err.contains("reference definition"), "{err}");
    }

    #[test]
    fn a_definition_kept_verbatim_inside_a_payload_passes() {
        let u = unit(BlockKind::Blockquote, QUOTED_DEF);
        assert!(
            check_inline(
                &policy(Some(true), None),
                &u,
                &unit_result("> [ref]: https://example.com/\n>\n> 정의 보유자."),
                &doc_refs(BLOCKQUOTE_DEF),
            )
            .is_ok()
        );
    }

    #[test]
    fn preserve_urls_false_permits_a_rewritten_definition() {
        let u = unit(BlockKind::Blockquote, QUOTED_DEF);
        assert!(
            check_inline(
                &policy(Some(false), None),
                &u,
                &unit_result("> [ref]: https://evil.example/\n>\n> 정의 보유자."),
                &doc_refs(BLOCKQUOTE_DEF),
            )
            .is_ok()
        );
    }

    #[test]
    fn prose_that_merely_looks_like_a_label_is_not_a_definition() {
        // `a[i]: x` carries `]:` and defines nothing on either side.
        let u = unit(BlockKind::Paragraph, "Index a[i]: the value.");
        assert!(
            check_inline(
                &policy(Some(true), None),
                &u,
                &unit_result("인덱스 a[i]: 값."),
                &doc_refs("Index a[i]: the value.\n"),
            )
            .is_ok()
        );
    }

    /// Labels whose comrak key is not itself a label comrak reads back to the
    /// same key: an edge of Unicode whitespace (trimmed by nothing, collapsed
    /// to an ASCII space) and a `\` left last once the space after it is
    /// trimmed (it would escape the closing `]`).
    const EDGE_LABELS: &[&str] = &["foo\u{a0}", "\u{3000}참고", "a\\ "];

    #[test]
    fn dropped_reference_to_a_gap_definition_with_an_edge_label_is_rejected() {
        for label in EDGE_LABELS {
            let source = format!("Read [docs][{label}].");
            let src = format!("{source}\n\n[{label}]: https://example.com/g\n");
            let err = check_inline(
                &policy(Some(true), None),
                &unit(BlockKind::Paragraph, &source),
                &unit_result("Read docs."),
                &doc_refs(&src),
            )
            .expect_err("the paragraph's one link is gone from the translation");
            assert!(err.contains("count changed"), "{label:?}: {err}");
        }
    }

    #[test]
    fn dropped_reference_to_a_container_definition_with_an_edge_label_is_rejected() {
        for label in EDGE_LABELS {
            let source = format!("Read [docs][{label}].");
            let src = format!("> [{label}]: https://example.com/q\n>\n> holder\n\n{source}\n");
            let err = check_inline(
                &policy(Some(true), None),
                &unit(BlockKind::Paragraph, &source),
                &unit_result("Read docs."),
                &doc_refs(&src),
            )
            .expect_err("the paragraph's one link is gone from the translation");
            assert!(err.contains("count changed"), "{label:?}: {err}");
        }
    }

    #[test]
    fn a_rewritten_definition_with_an_edge_label_is_rejected() {
        for label in EDGE_LABELS {
            let carrier = format!("> [{label}]: https://example.com/q\n>\n> holder");
            let src = format!("{carrier}\n\nRead [docs][{label}].\n");
            let err = check_inline(
                &policy(Some(true), None),
                &unit(BlockKind::Blockquote, &carrier),
                &unit_result(&format!("> [{label}]: https://evil.example/\n>\n> 보유자")),
                &doc_refs(&src),
            )
            .expect_err("the definition's destination was rewritten");
            assert!(err.contains("reference definition"), "{label:?}: {err}");
            assert!(err.contains("https://evil.example/"), "{label:?}: {err}");
        }
    }

    /// A label spread over many lines: inside comrak's 1,000-byte label limit
    /// once each continuation line's indentation is stripped, far past it as
    /// raw text.
    fn long_label(line_break: &str) -> String {
        (0..58)
            .map(|i| format!("word{i:02}xxxxxxxxxx"))
            .collect::<Vec<_>>()
            .join(line_break)
    }

    #[test]
    fn dropped_reference_to_a_long_label_spread_over_lines_is_rejected() {
        let source = format!("Read [docs][{}].", long_label(" "));
        for defs in [
            format!("[{}]: https://example.com/g\n", long_label("\n        ")),
            format!(
                "- outer\n  - [{}]: https://example.com/g\n",
                long_label("\n    ")
            ),
            format!(
                "> > > > [{}]: https://example.com/g\n",
                long_label("\n> > > > ")
            ),
        ] {
            let err = check_inline(
                &policy(Some(true), None),
                &unit(BlockKind::Paragraph, &source),
                &unit_result("Read docs."),
                &doc_refs(&format!("{source}\n\n{defs}")),
            )
            .expect_err("the paragraph's one link is gone from the translation");
            assert!(err.contains("count changed"), "{defs:?}: {err}");
        }
    }

    #[test]
    fn a_rewritten_definition_with_a_long_label_spread_over_lines_is_rejected() {
        let label = long_label("\n>         ");
        let carrier = format!("> [{label}]: https://example.com/q\n>\n> holder");
        let src = format!("{carrier}\n\nRead [docs][{}].\n", long_label(" "));
        let err = check_inline(
            &policy(Some(true), None),
            &unit(BlockKind::Blockquote, &carrier),
            &unit_result(&format!("> [{label}]: https://evil.example/\n>\n> 보유자")),
            &doc_refs(&src),
        )
        .expect_err("the definition's destination was rewritten");
        assert!(err.contains("reference definition"), "{err}");
        assert!(err.contains("https://evil.example/"), "{err}");
    }

    /// Blockquotes of two definitions whose labels continue across lines,
    /// where the first one's title, read as inline text, would open a code
    /// span, an inline link's title, an image's title or inline HTML that
    /// runs past the second label's `[`.
    const HIDDEN_LABEL_CARRIERS: &[&str] = &[
        "> [a\n> a]: /a \"`\"\n> [b\n> c]: https://example.com/b \"`\"\n>\n> holder",
        "> [a\n> a]: /a \"[x](/u '\"\n> [b\n> c]: https://example.com/b \"')\"\n>\n> holder",
        "> [a\n> a]: /a \"![x](/u '\"\n> [b\n> c]: https://example.com/b \"')\"\n>\n> holder",
        "> [a\n> a]: /a \"<span title='\"\n> [b\n> c]: https://example.com/b \"'>\"\n>\n> holder",
    ];

    #[test]
    fn dropped_reference_to_a_label_behind_an_inline_construct_is_rejected() {
        for carrier in HIDDEN_LABEL_CARRIERS {
            let err = check_inline(
                &policy(Some(true), None),
                &unit(BlockKind::Paragraph, "See [y][b c]."),
                &unit_result("See y."),
                &doc_refs(&format!("{carrier}\n\nSee [y][b c].\n")),
            )
            .expect_err("the paragraph's one link is gone from the translation");
            assert!(err.contains("count changed"), "{carrier:?}: {err}");
        }
    }

    #[test]
    fn a_rewritten_definition_behind_an_inline_construct_is_rejected() {
        for carrier in HIDDEN_LABEL_CARRIERS {
            let err = check_inline(
                &policy(Some(true), None),
                &unit(BlockKind::Blockquote, carrier),
                &unit_result(
                    &carrier
                        .replace("https://example.com/b", "https://evil.example/")
                        .replace("holder", "보유자"),
                ),
                &doc_refs(&format!("{carrier}\n\nSee [y][b c].\n")),
            )
            .expect_err("the definition's destination was rewritten");
            assert!(err.contains("reference definition"), "{carrier:?}: {err}");
            assert!(err.contains("https://evil.example/"), "{carrier:?}: {err}");
        }
    }

    #[test]
    fn a_payload_ending_in_an_unterminated_construct_still_resolves_its_references() {
        // The append once had to reason about a payload whose unterminated
        // HTML comment or fence swallowed the appended pool. Callback
        // resolution appends nothing: the reference ahead of the construct
        // resolves, and the construct's literal content stays literal.
        let refs = doc_refs(REF_POOL);
        let budget = crate::refmap::expansion_budget(&[]);
        for payload in [
            "See [x][ref].\n\n<!-- an unterminated [y][ref] comment\n",
            "See [x][ref].\n\n```\nunterminated [y][ref] fence\n",
        ] {
            assert_eq!(
                inline_inventory(payload, &refs, budget).destinations,
                vec![(DestKind::Link, "https://example.com/r".to_string())],
                "{payload:?}"
            );
            let u = unit(BlockKind::Paragraph, payload);
            assert!(
                check_inline(&policy(Some(true), None), &u, &unit_result(payload), &refs).is_ok()
            );
        }
    }
}
