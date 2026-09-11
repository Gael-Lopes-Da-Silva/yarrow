//! `yarrow fmt` - thin in-process wrapper around `yarrow_fmt::run_fmt`.

use std::process::ExitCode;

use yarrow_fmt::{FmtInput, run_fmt};

/// Format `.yar` files (same behavior as the `yarrow-fmt` binary).
pub fn run_fmt_command(input: FmtInput) -> ExitCode {
    run_fmt("yarrow fmt", input)
}
