//! `transync` command-line entrypoint.
//!
//! TRACE: SCN-12
//! TRACE: contracts.md §6

mod direction;
pub mod error;
mod logging;
mod output;
mod serve_cmd;
mod translate_cmd;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "transync",
    version,
    about = "GFM Markdown and HTML document translation with block-level sync."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Translate a Markdown or HTML document end-to-end.
    ///
    /// TRACE: SCN-12
    Translate(Box<translate_cmd::TranslateArgs>),

    /// Serve the demo HTML bundle on `127.0.0.1`.
    ///
    /// TRACE: SCN-13
    Serve(serve_cmd::ServeArgs),
}

#[tokio::main]
async fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            // Distinguish "user asked for --help/--version" from real
            // argument errors. The former exits 0; the latter exits 1
            // per `contracts.md` §6.
            let kind = e.kind();
            if matches!(
                kind,
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                e.print().ok();
                std::process::exit(error::ExitCode::Success as i32);
            } else {
                e.print().ok();
                std::process::exit(error::ExitCode::ArgumentError as i32);
            }
        }
    };
    // R0001-0032: the library members emit their operational diagnostics on
    // `tracing` and install nothing; this binary is where those events get a
    // destination. Installed after parsing (nothing has run yet, so nothing
    // is missed) and before any library call.
    logging::init(match &cli.cmd {
        Cmd::Translate(args) => logging::Verbosity::from_flags(args.quiet, args.verbose),
        // `serve` carries no verbosity flags; the default floor applies.
        Cmd::Serve(_) => logging::Verbosity::Default,
    });

    let exit_code = match cli.cmd {
        Cmd::Translate(args) => translate_cmd::run(*args).await,
        Cmd::Serve(args) => serve_cmd::run(args).await,
    };
    std::process::exit(exit_code);
}
