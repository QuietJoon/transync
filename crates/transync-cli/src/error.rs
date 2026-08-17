//! CLI exit codes per `docs/architecture/contracts.md` §6.
//!
//! TRACE: contracts.md §6

use transync::{TranslatorError, TransyncError};

/// Stable exit codes surfaced by the CLI binary. All variants are live, and
/// three modules produce them. `main` maps clap argument failures to
/// `ArgumentError` (and `--help`/`--version` to `Success`) for either
/// subcommand. `translate_cmd::run` surfaces the rest per `contracts.md` §6,
/// including `AllUnitsFellBack` when every translatable unit fell back to
/// source, and it is the only producer of `AllUnitsFellBack`, `WriteFailure`,
/// `ConfigurationRejected` and `DocumentRefused`. `serve_cmd::run` produces
/// `InputReadFailure` when `--rendered` does not name a readable directory,
/// `Other` when the address cannot be bound, and `Success` when a signal
/// stops a running server — the four codes `contracts.md` §6 records as
/// `serve`'s, counting clap's `ArgumentError`.
///
/// The numbers are **append-only**: a code never changes meaning and is
/// never repurposed, because they are what a calling script branches on and
/// nothing tells that script its meaning moved. `ConfigurationRejected` and
/// `DocumentRefused` were added in the v0.4.0 window (ti `e62b59`) and took
/// the next two free numbers for that reason — every code `0..=5` means
/// exactly what it meant before.
///
/// TRACE: SCN-12
/// TRACE: ti e62b59
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    ArgumentError = 1,
    InputReadFailure = 2,
    AllUnitsFellBack = 3,
    WriteFailure = 4,
    Other = 5,
    /// The provider refused the run because of **how it was configured** —
    /// a credential it would not accept, a request it rejected outright, or
    /// a token budget that could not hold the work. Changing the
    /// configuration and re-running is the remediation, and it is a
    /// *different* remediation from every other non-zero code, which is the
    /// whole reason this number exists.
    ///
    /// Deliberately not folded into `ArgumentError` (1). That code means the
    /// CLI itself refused the arguments before anything ran; this one means
    /// the arguments were well-formed, the run started, and the provider
    /// then rejected it. A script that retries after fixing its own argv
    /// must not be told those are the same event.
    ConfigurationRejected = 6,
    /// The provider refused **this document's content** — its content policy
    /// stopped generation, or the model declined and said so. No
    /// configuration change helps: transync's retry is a verbatim
    /// resubmission (ADR-0009), and identical content is what was refused.
    /// The correct automated response is to skip this document, which is why
    /// it cannot share a number with `ConfigurationRejected`.
    DocumentRefused = 7,
}

impl ExitCode {
    /// The one classification table from a failed pipeline run to the number
    /// the process surfaces.
    ///
    /// It lives beside the enum rather than at the call site because the
    /// numbers are a contract (`contracts.md` §6) and a reader who comes here
    /// for the contract should find the whole mapping, not half of it.
    ///
    /// `TransyncError` and `TranslatorError` are both `#[non_exhaustive]`, so
    /// each match needs a wildcard arm. That arm answers [`Self::Other`],
    /// which is the honest answer: a failure cause this build does not know
    /// about has no remediation this build can name. Codes are therefore
    /// added by *moving causes out of 5*, never by re-meaning a number.
    ///
    /// TRACE: contracts.md §6
    /// TRACE: ti e62b59
    pub(crate) fn for_pipeline_failure(err: &TransyncError) -> Self {
        match err {
            // contracts.md §6: a source the parser refuses is an input
            // failure, in the same family as an unreadable `--input`.
            TransyncError::Parse(_) => ExitCode::InputReadFailure,
            TransyncError::Translator(e) => Self::for_translator_failure(e),
            _ => ExitCode::Other,
        }
    }

    /// The provider half of the table (ti `e62b59`, over the DCR-0023 /
    /// DCR-0029 taxonomy).
    ///
    /// The split is by **what the operator must do next**, which is the only
    /// question a process exit code can usefully answer. The taxonomy itself
    /// deliberately encodes *cause* and not remediation (`contracts.md` §1),
    /// so this function is where the one becomes the other — and it is the
    /// CLI's policy over the taxonomy, not a property of it. A library
    /// consumer reads `stable_code()` and decides its own.
    fn for_translator_failure(err: &TranslatorError) -> Self {
        match err {
            // Fix the configuration and re-run.
            //
            // `Authentication` — the key. `ProviderRejected` — the request
            // itself was refused; `status: Some(404)` is the mistyped
            // `--model`, and the sibling 4xx cases are equally about what was
            // sent rather than what it contained. `OutputCeilingExhausted`
            // points at `[batching].target_output_tokens`; the adjacent
            // `ContextWindowExceeded` points at the `[batching]` input budget
            // and `max_units_per_batch`. Different knobs, one action.
            TranslatorError::Authentication(_)
            | TranslatorError::ProviderRejected { .. }
            | TranslatorError::OutputCeilingExhausted(_)
            | TranslatorError::ContextWindowExceeded(_) => ExitCode::ConfigurationRejected,

            // Skip this document and move on.
            TranslatorError::ContentFiltered(_) | TranslatorError::ModelRefused(_) => {
                ExitCode::DocumentRefused
            }

            // Everything else stays 5, and each for a reason rather than by
            // omission. `Network` / `RateLimited` reach here only with the
            // pipeline's retry budget already spent, so the run is out of
            // automated options but nothing about the configuration or the
            // document is implicated. `MalformedResponse` and
            // `ResponseTooLarge` are the provider misbehaving. `Unsupported`
            // is genuinely ambiguous — a different model might accept the
            // batch, or the document might carry a construct nothing
            // accepts — and a code that guesses is worse than one that
            // admits it does not know. `Cancelled` from a `Translator`'s own
            // cancellation source is a stop the caller arranged.
            // `Other` is unclassified by definition.
            _ => ExitCode::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers themselves. This test is the append-only rule made
    /// mechanical: changing any of these values is changing what a deployed
    /// script's `if [ $? -eq N ]` means, so it must be a deliberate edit here
    /// and not a side effect of reordering the enum.
    #[test]
    fn exit_code_numbers_are_frozen() {
        assert_eq!(ExitCode::Success as i32, 0);
        assert_eq!(ExitCode::ArgumentError as i32, 1);
        assert_eq!(ExitCode::InputReadFailure as i32, 2);
        assert_eq!(ExitCode::AllUnitsFellBack as i32, 3);
        assert_eq!(ExitCode::WriteFailure as i32, 4);
        assert_eq!(ExitCode::Other as i32, 5);
        assert_eq!(ExitCode::ConfigurationRejected as i32, 6);
        assert_eq!(ExitCode::DocumentRefused as i32, 7);
    }

    #[test]
    fn configuration_faults_exit_6() {
        for err in [
            TranslatorError::Authentication("bad key".into()),
            TranslatorError::ProviderRejected {
                status: Some(404),
                message: "no such model".into(),
            },
            TranslatorError::ProviderRejected {
                status: None,
                message: "refused".into(),
            },
            TranslatorError::OutputCeilingExhausted("cut off".into()),
            TranslatorError::ContextWindowExceeded("too long".into()),
        ] {
            let code = ExitCode::for_pipeline_failure(&TransyncError::Translator(err));
            assert_eq!(code, ExitCode::ConfigurationRejected);
        }
    }

    #[test]
    fn document_faults_exit_7() {
        for err in [
            TranslatorError::ContentFiltered("policy".into()),
            TranslatorError::ModelRefused("I will not translate this".into()),
        ] {
            let code = ExitCode::for_pipeline_failure(&TransyncError::Translator(err));
            assert_eq!(code, ExitCode::DocumentRefused);
        }
    }

    /// The residual is deliberate, so it is asserted rather than left to
    /// whatever the wildcard happens to do.
    #[test]
    fn unclassified_and_transport_faults_stay_5() {
        for err in [
            TranslatorError::Network("reset".into()),
            TranslatorError::RateLimited { retry_after: None },
            TranslatorError::MalformedResponse("not json".into()),
            TranslatorError::Unsupported("constraint".into()),
            TranslatorError::ResponseTooLarge("32 MiB".into()),
            TranslatorError::Cancelled,
            TranslatorError::Other("?".into()),
        ] {
            let code = ExitCode::for_pipeline_failure(&TransyncError::Translator(err));
            assert_eq!(code, ExitCode::Other);
        }
    }

    /// The two arms that were already in the table keep their answers: this
    /// change moves causes out of 5, it does not re-mean 2.
    #[test]
    fn engine_side_arms_are_unchanged() {
        let parse = TransyncError::Parse(transync::ParseError::TooDeeplyNested {
            depth: 200,
            limit: 128,
        });
        assert_eq!(
            ExitCode::for_pipeline_failure(&parse),
            ExitCode::InputReadFailure
        );
        assert_eq!(
            ExitCode::for_pipeline_failure(&TransyncError::Validation("nope".into())),
            ExitCode::Other
        );
        assert_eq!(
            ExitCode::for_pipeline_failure(&TransyncError::Cancelled),
            ExitCode::Other
        );
    }
}
