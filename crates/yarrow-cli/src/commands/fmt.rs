//! `yarrow fmt` - thin in-process wrapper around `yarrow_fmt::run_fmt`.
//!
//! Global `--color` / `-q` are no-ops here: `yarrow_fmt` has no color or quiet
//! knobs. Format payload and `--check` / I/O messages still print as today.

use std::process::ExitCode;

use yarrow_fmt::{FmtInput, run_fmt};

/// Format `.yar` files (same behavior as the `yarrow-fmt` binary).
pub fn run_fmt_command(input: FmtInput) -> ExitCode {
    run_fmt("yarrow fmt", input)
}
