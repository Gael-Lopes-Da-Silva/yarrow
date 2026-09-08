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

pub use args::{Cli, Cmd, TargetKind};

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
        (
            Some(Cmd::Run {
                file,
                target,
                main,
                program_args,
            }),
            None,
        ) => commands::run_file(&file, target, &main, &program_args, &cli.global),
        (None, Some(file)) => {
            // `yarrow <file>` is sugar for `yarrow run --target object <file>`.
            commands::run_file(&file, TargetKind::Object, "main", &[], &cli.global)
        }
        (
            Some(Cmd::Compile {
                file,
                target,
                emit,
                main,
                output,
            }),
            None,
        ) => commands::compile_file(&file, target, emit, &main, output.as_deref(), &cli.global),
        (Some(Cmd::Check { file, main }), None) => commands::check_file(&file, &main, &cli.global),
        (
            Some(Cmd::Interpret {
                file,
                main,
                program_args,
            }),
            None,
        ) => commands::interpret_file(&file, &main, &program_args, &cli.global),
        (Some(Cmd::Repl), None) => commands::run_repl(&cli.global),
        (
            Some(Cmd::Fmt {
                check,
                stdin,
                max_width,
                sort_requires,
                reorder_layout,
                paths,
            }),
            None,
        ) => commands::run_fmt_command(
            paths,
            check,
            stdin,
            max_width,
            sort_requires,
            reorder_layout,
        ),
        (
            Some(Cmd::Lsp {
                stdio,
                search_paths,
                main,
                no_format,
                log_level,
            }),
            None,
        ) => {
            if !stdio {
                eprintln!("error: only --stdio transport is supported");
                ExitCode::from(2)
            } else {
                commands::run_lsp(&cli.global, &search_paths, &main, !no_format, log_level)
            }
        }
        (Some(Cmd::Dump { file, emit }), None) => commands::dump_file(&file, emit, &cli.global),
        (Some(Cmd::Explain { code }), None) => commands::explain_code(&code, &cli.global),
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
            eprintln!(
                "usage: yarrow <file.yar>\n       yarrow run [--target jit|object] <file.yar> [-- ARGS...]\n       yarrow compile [--target jit|object] <file.yar>\n       yarrow interpret <file.yar> [-- ARGS...]\n       yarrow repl\n       yarrow fmt [--check] [PATH...]\n       yarrow lsp"
            );
            ExitCode::from(2)
        }
    }
}
