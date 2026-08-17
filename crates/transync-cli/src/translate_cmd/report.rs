//! Human-facing output for `transync translate`: the quiet-gated stderr sink
//! plus pure renderers for every diagnostic and summary line the command
//! emits. Keeping the prose here makes it reviewable in one place and keeps
//! `execute` about control flow.
//!
//! TRACE: contracts.md §6

use transync::{AutoGlossaryReport, AutoGlossaryStatus, ValidationReport, ValidationSummary};

/// The command's stderr channel. Every diagnostic goes through here so the
/// `--quiet` gate is applied in exactly one place; the `transync: ` prefix
/// is part of the contract with the smoke tests.
pub(crate) struct Reporter {
    quiet: bool,
}

impl Reporter {
    pub(crate) fn new(quiet: bool) -> Self {
        Self { quiet }
    }

    /// Emit one diagnostic line, unless `--quiet` was given.
    pub(crate) fn say(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{}", diagnostic_line(msg));
        }
    }
}

/// Render one `transync: ` line: the prefix, then `msg` with every C0/C1
/// control character escaped.
///
/// **Why anything is escaped at all.** Architectural invariant 7 makes source
/// Markdown untrusted data, and most of what reaches this stream is derived
/// from it or from a remote server: a provider's error body (`translation
/// failed: {e}` carries HTTP-body-derived text), a model-authored
/// `rejection_reason` or `warning` a source document can steer, a path or a
/// title that came out of the document. None of it is this program's prose.
/// A raw `\n` in it **forges a second `transync: ` line** — a diagnostic the
/// run never emitted, which is exactly what a log-reading operator or script
/// trusts the prefix to rule out — and a raw `\x1b[` reaches the terminal as a
/// control sequence rather than as text. R0002-0068 bounded this text in count
/// and in bytes (`validate::bounded_warnings`, contracts.md §3a); nothing
/// bounded its *alphabet*, and that was the third of its three asks
/// (R0004-0071).
///
/// **Why these characters.** Every diagnostic here is one line by contract, so
/// nothing in C0 (below `0x20`, plus `DEL`) or C1 (`0x80`–`0x9f`, which some
/// terminals still decode as controls) has a legitimate reason to appear. The
/// three familiar ones become `\n` / `\r` / `\t` because that is how a reader
/// recognizes them; anything else becomes `\u{XX}`. Nothing is dropped — the
/// escaped form carries the same information, inert, and an operator can still
/// see exactly what the provider sent.
pub(crate) fn diagnostic_line(msg: &str) -> String {
    const PREFIX: &str = "transync: ";
    let mut out = String::with_capacity(PREFIX.len() + msg.len());
    out.push_str(PREFIX);
    for ch in msg.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:02x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The stderr lines a completed pipeline run produces, in emission order.
///
/// - R0008-0013: every top-level source node that survives into the
///   translated Markdown but is not rendered as ordinary translated content
///   (footnote definitions, front matter, …) so the handling is visible, not
///   silent. Spec 2026-08-03 §3.2 adds html blocks with nothing translatable
///   (or a failed extraction) to the same channel.
/// - OI-0026: a silently degraded run of an explicitly requested feature
///   would be misleading, so a failed extraction is reported for every
///   non-quiet run, not only under `--verbose`.
/// - D1 §2.3: say that units risk truncating the provider response (and
///   aborting the run), and in what flag terms to fix it.
///
/// R0001-0032 note on what is *not* here. `transync-core` emits one
/// `transync::pipeline` `warn` per over-ceiling unit, and since the binary
/// installs a subscriber ([`crate::logging`]) those reach the same stderr.
/// This function used to repeat each of them verbatim with a remediation
/// suffix, which printed the identical sentence twice per flagged unit — on
/// the SCN-14 fixture at a small ceiling, thirty lines carrying fifteen
/// facts. The per-unit naming belongs to the channel that already does it
/// (and to `--validation-report`); what only the CLI knows is which *flags*
/// move the ceiling, so that is the one line it adds.
pub(crate) fn pipeline_diagnostics(
    report: &ValidationReport,
    alignment_map: &transync::AlignmentMap,
) -> Vec<String> {
    let mut lines = Vec::new();
    for note in &report.skipped_source_nodes {
        lines.push(format!("note: {note}"));
    }
    if let Some(ag) = &report.auto_glossary
        && ag.status == AutoGlossaryStatus::Failed
    {
        lines.push(format!("warning: {}", format_auto_glossary_summary(ag)));
    }
    if !report.output_budget_warnings.is_empty() {
        // DCR-0026: a flagged unit that is a whole TABLE has one remedy the
        // others do not — the row-window split — so the line names it, and
        // only then. The join goes through the alignment map because that is
        // where a block's kind is on the CLI's side of the boundary; a flagged
        // row *window* is not in it, which is correct: a window's table was
        // already split, so the strategy is not the answer for it.
        let flagged_table = report.output_budget_warnings.iter().any(|w| {
            alignment_map
                .blocks
                .iter()
                .any(|b| b.source_block_id == w.unit_id && b.block_kind == "table")
        });
        let table_remedy = if flagged_table {
            ", --table-strategy row-window-first (tables only)"
        } else {
            ""
        };
        lines.push(format!(
            "warning: {} unit(s) estimated over the per-batch output ceiling (each named \
             on the transync::pipeline warning channel and in --validation-report) — raise \
             --target-output-tokens, lower --output-expansion-factor{table_remedy}, or split \
             the source block",
            report.output_budget_warnings.len()
        ));
    }
    lines
}

/// The `--verbose` validation tally (one line, no `transync: ` prefix).
pub(crate) fn verbose_tally(summary: &ValidationSummary) -> String {
    format!(
        "total_units={} translated={} preserved={} partial={} fallback={} retried={}",
        summary.total_units,
        summary.translated,
        summary.preserved,
        summary.partially_translated,
        summary.fallback_source,
        summary.retried_units,
    )
}

/// One-line human summary of the auto-glossary preflight outcome, used by
/// `--verbose` and (for `Failed`) by the stderr notice.
///
/// TRACE: OI-0026
pub(crate) fn format_auto_glossary_summary(report: &AutoGlossaryReport) -> String {
    use AutoGlossaryStatus as S;
    match report.status {
        S::Extracted => {
            let mut notes: Vec<String> = Vec::new();
            if report.dropped_conflicts > 0 {
                notes.push(format!(
                    "{} conflict(s) deferred to profile",
                    report.dropped_conflicts
                ));
            }
            if report.dropped_invalid > 0 {
                notes.push(format!("{} dropped as invalid", report.dropped_invalid));
            }
            if report.source_truncated {
                notes.push("source truncated".to_string());
            }
            let suffix = if notes.is_empty() {
                String::new()
            } else {
                format!(" ({})", notes.join(", "))
            };
            format!(
                "auto-glossary: {} term(s) merged{suffix}",
                report.accepted_terms
            )
        }
        S::Unsupported => {
            "auto-glossary: this provider does not support extraction; static glossary only"
                .to_string()
        }
        S::Failed => format!(
            "auto-glossary: extraction failed ({}); static glossary only",
            report.error.as_deref().unwrap_or("no diagnostic")
        ),
        S::Skipped => "auto-glossary: skipped — the document has no translatable block".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R0004-0071: the text on this stream is largely not this program's.
    /// A provider's error body and a model-authored rejection reason both
    /// reach it, and invariant 7 says a source document can steer the latter.
    /// A raw newline in that text forges a `transync: ` line the run never
    /// emitted; a raw ESC drives the terminal. Both are text now, and the
    /// information survives the escaping.
    #[test]
    fn a_diagnostic_is_one_line_and_carries_no_terminal_controls() {
        let forged = "translation failed: rejected\ntransync: outputs written to /tmp/evil";
        let line = diagnostic_line(forged);

        assert_eq!(
            line.lines().count(),
            1,
            "one line, whatever arrived: {line}"
        );
        assert_eq!(
            line.lines().filter(|l| l.starts_with("transync: ")).count(),
            1,
            "only the prefix this run wrote may read as a transync line: {line}"
        );
        assert!(
            line.contains("\\ntransync: outputs"),
            "the forged newline survives as text: {line}"
        );

        let escaped = diagnostic_line("bell \x07 esc \x1b[31mred\x1b[0m del \x7f c1 \u{9b}");
        for raw in ['\x07', '\x1b', '\x7f', '\u{9b}'] {
            assert!(
                !escaped.contains(raw),
                "no control character may reach the terminal: {escaped:?}"
            );
        }
        assert!(
            escaped.contains("\\u{1b}[31mred") && escaped.contains("\\u{7f}"),
            "escaped, not dropped — the operator still sees what was sent: {escaped}"
        );
        assert!(
            escaped.contains("\\u{9b}"),
            "the C1 block is a control block too: {escaped}"
        );

        // Ordinary prose is untouched, non-ASCII included: the alphabet rule
        // is about control characters, not about anything unfamiliar.
        assert_eq!(
            diagnostic_line("note: 12 term(s) merged — 존댓말"),
            "transync: note: 12 term(s) merged — 존댓말"
        );
        assert_eq!(
            diagnostic_line("tabbed\there"),
            "transync: tabbed\\there",
            "a tab is a control character with a familiar spelling"
        );
    }

    fn glossary_row(
        status: AutoGlossaryStatus,
        accepted: u32,
        conflicts: u32,
        invalid: u32,
        truncated: bool,
        error: Option<String>,
    ) -> AutoGlossaryReport {
        AutoGlossaryReport {
            status,
            accepted_terms: accepted,
            dropped_conflicts: conflicts,
            dropped_invalid: invalid,
            source_truncated: truncated,
            error,
            terms: Vec::new(),
        }
    }

    /// OI-0026: the `--verbose` line names what the preflight contributed
    /// and, for the degraded statuses, that the run fell back to the static
    /// glossary.
    #[test]
    fn auto_glossary_summary_names_counts_and_degradation() {
        let merged = format_auto_glossary_summary(&glossary_row(
            AutoGlossaryStatus::Extracted,
            12,
            1,
            0,
            true,
            None,
        ));
        assert!(merged.contains("12 term(s) merged"), "{merged}");
        assert!(
            merged.contains("1 conflict(s) deferred to profile"),
            "{merged}"
        );
        assert!(merged.contains("source truncated"), "{merged}");

        let clean = format_auto_glossary_summary(&glossary_row(
            AutoGlossaryStatus::Extracted,
            3,
            0,
            0,
            false,
            None,
        ));
        assert_eq!(clean, "auto-glossary: 3 term(s) merged");

        let failed = format_auto_glossary_summary(&glossary_row(
            AutoGlossaryStatus::Failed,
            0,
            0,
            0,
            false,
            Some("network: connection reset".to_string()),
        ));
        assert!(failed.contains("network: connection reset"), "{failed}");
        assert!(failed.contains("static glossary only"), "{failed}");

        let unsupported = format_auto_glossary_summary(&glossary_row(
            AutoGlossaryStatus::Unsupported,
            0,
            0,
            0,
            false,
            None,
        ));
        assert!(
            unsupported.contains("does not support extraction"),
            "{unsupported}"
        );

        let skipped = format_auto_glossary_summary(&glossary_row(
            AutoGlossaryStatus::Skipped,
            0,
            0,
            0,
            false,
            None,
        ));
        assert!(skipped.contains("no translatable block"), "{skipped}");
    }

    /// The post-run diagnostics keep their emission order (skipped-node
    /// notes first, then the auto-glossary degradation notice), and a
    /// successful preflight stays silent outside `--verbose`.
    #[test]
    fn pipeline_diagnostics_order_and_glossary_gating() {
        let mut report = ValidationReport::default();
        report.skipped_source_nodes = vec!["front matter preserved".to_string()];
        report.auto_glossary = Some(glossary_row(
            AutoGlossaryStatus::Failed,
            0,
            0,
            0,
            false,
            Some("boom".to_string()),
        ));
        let lines = pipeline_diagnostics(&report, &alignment_map(&[]));
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0], "note: front matter preserved");
        assert!(lines[1].starts_with("warning: auto-glossary:"), "{lines:?}");

        report.auto_glossary = Some(glossary_row(
            AutoGlossaryStatus::Extracted,
            4,
            0,
            0,
            false,
            None,
        ));
        let lines = pipeline_diagnostics(&report, &alignment_map(&[]));
        assert_eq!(
            lines.len(),
            1,
            "a successful preflight reports only under --verbose: {lines:?}"
        );
    }

    /// R0001-0032: however many units the output-budget preflight flags, the
    /// CLI contributes exactly ONE line — the per-unit sentences are the
    /// `transync::pipeline` warning channel's job now that a subscriber
    /// exists to carry them. The line still names the count and both
    /// remediation flags, which is what a log record cannot know.
    #[test]
    fn output_budget_warnings_collapse_to_one_remediation_line() {
        let mut report = ValidationReport::default();
        assert!(
            pipeline_diagnostics(&report, &alignment_map(&[])).is_empty(),
            "no flagged unit means no line at all"
        );

        report.output_budget_warnings = vec![
            budget_warning("h1-0001", 90),
            budget_warning("p-0002", 339),
            budget_warning("t-0012", 289),
        ];
        let lines = pipeline_diagnostics(&report, &alignment_map(&[]));
        assert_eq!(lines.len(), 1, "three flagged units, one line: {lines:?}");
        assert!(lines[0].contains("3 unit(s)"), "{}", lines[0]);
        assert!(lines[0].contains("--target-output-tokens"), "{}", lines[0]);
        assert!(
            lines[0].contains("--output-expansion-factor"),
            "{}",
            lines[0]
        );
    }

    fn budget_warning(unit_id: &str, estimated: u32) -> transync::OutputBudgetWarning {
        transync::OutputBudgetWarning {
            unit_id: transync::BlockId(unit_id.to_string()),
            estimated_output_tokens: estimated,
            ceiling_tokens: 60,
        }
    }

    /// An alignment map carrying one row per `(id, kind)` — the join
    /// `pipeline_diagnostics` uses to tell a flagged table from a flagged
    /// paragraph. Empty by default, which is the "no table flagged" case.
    fn alignment_map(blocks: &[(&str, &str)]) -> transync::AlignmentMap {
        // Deserialized rather than struct-literal'd: `AlignmentMap` and
        // `AlignmentBlock` are `#[non_exhaustive]` (contracts §1), so this is
        // the construction route a consumer outside the engine actually has —
        // and it exercises the §3 wire shape at the same time.
        let rows: Vec<serde_json::Value> = blocks
            .iter()
            .enumerate()
            .map(|(i, (id, kind))| {
                serde_json::json!({
                    "source_block_id": id,
                    "target_block_id": id,
                    "block_kind": kind,
                    "source_order": i,
                    "target_order": i,
                    "source_range": { "start": 0, "end": 0 },
                    "target_range": { "start": 0, "end": 0 },
                    "sync_role": "anchor",
                    "fallback_status": "translated",
                    "parent_id": null,
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "schema_version": transync::ALIGNMENT_SCHEMA_VERSION,
            "document_id": "0",
            "source_language": "en",
            "target_language": "ko",
            "detected_source_language": null,
            "generator": {
                "name": "transync",
                "version": "0",
                "prompt_version": "0",
                "schema_version": "0",
                "model": "test",
            },
            "blocks": rows,
            "validation_summary": {
                "total_units": 0,
                "translated": 0,
                "preserved": 0,
                "partially_translated": 0,
                "fallback_source": 0,
                "retried_units": 0,
            },
        }))
        .expect("the fixture matches the §3 wire shape")
    }

    /// DCR-0026: the row-window split is a remedy only a TABLE has, so the
    /// line names `--table-strategy` exactly when a flagged unit is one.
    #[test]
    fn the_table_strategy_remedy_is_named_only_for_a_flagged_table() {
        let mut report = ValidationReport::default();
        report.output_budget_warnings = vec![budget_warning("p-0002", 339)];

        let without = pipeline_diagnostics(
            &report,
            &alignment_map(&[("p-0002", "paragraph"), ("t-0012", "table")]),
        );
        assert_eq!(without.len(), 1);
        assert!(
            !without[0].contains("--table-strategy"),
            "a flagged paragraph gets no table remedy: {}",
            without[0]
        );

        report
            .output_budget_warnings
            .push(budget_warning("t-0012", 900));
        let with = pipeline_diagnostics(
            &report,
            &alignment_map(&[("p-0002", "paragraph"), ("t-0012", "table")]),
        );
        assert_eq!(with.len(), 1, "still one line: {with:?}");
        assert!(
            with[0].contains("--table-strategy row-window-first"),
            "{}",
            with[0]
        );
        assert!(with[0].contains("--target-output-tokens"), "{}", with[0]);

        // A flagged row WINDOW is not in the alignment map at all — correct:
        // its table was already split, so the strategy is not its remedy.
        let mut windowed = ValidationReport::default();
        windowed.output_budget_warnings = vec![budget_warning("t-0012.w03", 900)];
        let lines = pipeline_diagnostics(&windowed, &alignment_map(&[("t-0012", "table")]));
        assert!(!lines[0].contains("--table-strategy"), "{}", lines[0]);
    }
}
