//! Yarrow command-line driver.
//!
//! This crate owns argument parsing, subcommands, stdout/stderr, and process
//! exit codes. All compile work is delegated to `yarrow_core`.
//!
//! # Entry point
//!
//! ```no_run
//! use yarrow_cli::run;
//! use std::process::ExitCode;
//!
//! fn main() -> ExitCode {
//!     run(std::env::args_os())
//! }
//! ```

mod args;
mod commands;
mod diagnostics;

pub use args::{Cli, Cmd};

use std::ffi::OsString;
use std::process::ExitCode;

/// Main entry point. Accepts anything convertible to an iterator of OS
/// strings so it is easy to test with `&[&str]` slices.
pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    use clap::Parser as _;
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            // Clap writes help/version to stdout and errors to stderr.
            err.print().ok();
            let code = if err.use_stderr() { 2 } else { 0 };
            return ExitCode::from(code);
        }
    };

    match (cli.cmd, cli.file) {
        (Some(Cmd::Run { file }), None) => commands::run_file(&file, &cli.global),
        (None, Some(file)) => commands::run_file(&file, &cli.global),
        (Some(Cmd::Check { file }), None) => commands::check_file(&file, &cli.global),
        (Some(Cmd::Version), None) => {
            // Clap's built-in `--version` prints only the top-level crate version;
            // this subcommand matches `rustc -V` style UX.
            println!("yarrow {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        (Some(_), Some(_)) => {
            // Shouldn't happen in practice (clap would accept it), but keep it
            // deterministic.
            eprintln!("error: provide either a subcommand or a single FILE argument");
            ExitCode::from(2)
        }
        (None, None) => {
            // `arg_required_else_help` isn't enough once everything is optional.
            // Print a concise usage and keep exit code consistent.
            eprintln!("usage: yarrow <file.yar>\n       yarrow run <file.yar>");
            ExitCode::from(2)
        }
    }
}
