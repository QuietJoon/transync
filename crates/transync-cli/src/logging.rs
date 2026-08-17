//! Where the library's `tracing` events go in the reference binary.
//!
//! `transync-core` and `transync-openai` report every operational
//! degradation on the `tracing` channel — cache failures treated as misses,
//! provider backoff, batch-schema-fault routing, full-reparse fallback
//! cascades, conflicting per-batch language detection, profile load
//! warnings, an unusable `TRANSYNC_OPENAI_API` override. Those are `warn`
//! and `info` records, not return values, precisely because a *library*
//! must not decide whether an operator sees them. Until R0001-0032 the
//! reference binary never made that decision either, so every one of those
//! records went to a no-op sink and the operator saw nothing.
//!
//! This module is that decision, and it is the only place in the workspace
//! that makes it: the binary installs one `fmt` subscriber writing to
//! **stderr** (stdout belongs to the artifacts), at a level derived from the
//! same `--quiet` / `--verbose` posture that already governs the command's
//! own diagnostics.
//!
//! TRACE: contracts.md §6

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

/// How loud the process is on stderr, resolved from the subcommand's flags.
///
/// The three arms are the three the CLI already exposes; there is no
/// separate logging flag, because a second verbosity knob that disagreed
/// with `--quiet` would be a worse contract than no knob at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verbosity {
    /// `--quiet`: nothing at all. The command's own diagnostics are already
    /// suppressed here, and a subscriber that kept talking would defeat the
    /// flag.
    Quiet,
    /// No flag: `warn` and above — the operational degradations, and only
    /// those.
    Default,
    /// `--verbose`: `debug` and above, which adds the `info` records — today
    /// the auto-glossary preflight's outcome, including the "this provider
    /// cannot extract at all" case no rendered CLI line covers.
    Verbose,
}

impl Verbosity {
    /// Map the mutually-exclusive `--quiet` / `--verbose` pair. Clap rejects
    /// both at parse time, so the ordering below is not a precedence policy.
    pub(crate) fn from_flags(quiet: bool, verbose: bool) -> Self {
        if quiet {
            Self::Quiet
        } else if verbose {
            Self::Verbose
        } else {
            Self::Default
        }
    }

    /// The floor this verbosity puts under the filter.
    fn level(self) -> LevelFilter {
        match self {
            Self::Quiet => LevelFilter::OFF,
            Self::Default => LevelFilter::WARN,
            Self::Verbose => LevelFilter::DEBUG,
        }
    }
}

/// Install the process-wide stderr subscriber. Call once, after argument
/// parsing (nothing emits before that) and before any library call.
///
/// `RUST_LOG` is applied **on top of** the flag-derived level rather than
/// replacing it, so the usual per-target directive
/// (`RUST_LOG=transync::pipeline=trace`) widens one target and leaves the
/// rest at their `warn` floor. `--quiet` ignores `RUST_LOG` entirely: an
/// operator who asked for silence should not have to also scrub their
/// environment to get it.
pub(crate) fn init(verbosity: Verbosity) {
    tracing_subscriber::fmt()
        .with_env_filter(filter_for(verbosity))
        .with_writer(std::io::stderr)
        // No wall-clock stamp: these lines interleave with the command's own
        // `transync: ` diagnostics on one stderr, and a timestamp column
        // would make that stream harder to read, not easier. `RUST_LOG`
        // users who want timing have the tracing ecosystem for it.
        .without_time()
        .init();
}

/// Read `RUST_LOG` and hand it to [`filter_from`]. The env read is isolated
/// here so the policy below stays a pure function: a test that had to
/// `set_var` its way to coverage would be mutating a process-global from one
/// thread while every other test's `getenv` ran on another.
fn filter_for(verbosity: Verbosity) -> EnvFilter {
    filter_from(
        verbosity,
        std::env::var(EnvFilter::DEFAULT_ENV).ok().as_deref(),
    )
}

/// Build the filter: the verbosity floor, then any `RUST_LOG` directives
/// layered over it.
///
/// An unparsable directive is reported and skipped rather than dropping the
/// whole variable — a typo in one directive should cost the operator that
/// directive, not every diagnostic they asked for.
fn filter_from(verbosity: Verbosity, rust_log: Option<&str>) -> EnvFilter {
    let mut filter = EnvFilter::default().add_directive(verbosity.level().into());
    if verbosity == Verbosity::Quiet {
        return filter;
    }
    let Some(raw) = rust_log else {
        return filter;
    };
    for directive in raw.split(',').map(str::trim).filter(|d| !d.is_empty()) {
        match directive.parse() {
            Ok(parsed) => filter = filter.add_directive(parsed),
            Err(e) => eprintln!(
                "transync: ignoring unparsable {} directive {directive:?}: {e}",
                EnvFilter::DEFAULT_ENV
            ),
        }
    }
    filter
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flag pair maps onto the three levels, and the no-flag default is
    /// the one that matters: it is what makes the previously-invisible
    /// `warn` records visible without `--verbose`.
    #[test]
    fn verbosity_maps_flags_to_levels() {
        assert_eq!(Verbosity::from_flags(false, false), Verbosity::Default);
        assert_eq!(Verbosity::from_flags(true, false), Verbosity::Quiet);
        assert_eq!(Verbosity::from_flags(false, true), Verbosity::Verbose);

        assert_eq!(Verbosity::Default.level(), LevelFilter::WARN);
        assert_eq!(Verbosity::Quiet.level(), LevelFilter::OFF);
        assert_eq!(Verbosity::Verbose.level(), LevelFilter::DEBUG);
    }

    /// With no `RUST_LOG`, each verbosity is exactly its floor.
    #[test]
    fn bare_environment_is_the_flag_derived_floor() {
        assert_eq!(filter_from(Verbosity::Default, None).to_string(), "warn");
        assert_eq!(filter_from(Verbosity::Verbose, None).to_string(), "debug");
        assert_eq!(filter_from(Verbosity::Quiet, None).to_string(), "off");
    }

    /// `RUST_LOG` layers over the floor instead of replacing it: a
    /// per-target directive widens that target and every other target keeps
    /// the flag-derived level. `--quiet` does not read it at all — an
    /// operator who asked for silence should not have to scrub their
    /// environment to get it.
    #[test]
    fn rust_log_layers_over_the_floor_and_quiet_ignores_it() {
        let widened = filter_from(Verbosity::Default, Some("transync::pipeline=trace")).to_string();
        assert!(
            widened.contains("transync::pipeline=trace"),
            "the directive must apply: {widened}"
        );
        assert!(
            widened.contains("warn"),
            "and the floor must survive it: {widened}"
        );

        let quiet = filter_from(Verbosity::Quiet, Some("transync::pipeline=trace")).to_string();
        assert_eq!(quiet, "off", "--quiet must not layer RUST_LOG: {quiet}");
    }

    /// A typo costs the operator that directive, not the whole variable:
    /// the good half of a comma list still applies.
    #[test]
    fn unparsable_directive_is_skipped_not_fatal() {
        let mixed = filter_from(
            Verbosity::Default,
            Some("transync::pipeline=trace, not a directive at all"),
        )
        .to_string();
        assert!(
            mixed.contains("transync::pipeline=trace"),
            "the parsable directive must survive its neighbor: {mixed}"
        );
        assert!(mixed.contains("warn"), "as must the floor: {mixed}");
    }
}
