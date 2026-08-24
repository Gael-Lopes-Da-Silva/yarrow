//! Implementation of the `explain` subcommand.

use std::process::ExitCode;

use crate::args::GlobalArgs;

/// Print the long-form help for a diagnostic code, or exit 2 if unknown.
pub fn explain_code(code: &str, global: &GlobalArgs) -> ExitCode {
    if global.verbose {
        eprintln!("looking up diagnostic {code}");
    }
    match yarrow_core::explain_code(code) {
        Some(entry) => {
            print!("{}", yarrow_core::format_explain(entry));
            ExitCode::SUCCESS
        }
        None => {
            let normalized = yarrow_core::normalize_code(code);
            eprintln!("error: unknown diagnostic code '{normalized}'");
            ExitCode::from(2)
        }
    }
}
