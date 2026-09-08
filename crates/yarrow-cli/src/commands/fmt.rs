//! `yarrow fmt` - thin in-process wrapper around `yarrow_fmt::run_fmt`.

use std::path::PathBuf;
use std::process::ExitCode;

use yarrow_fmt::{FmtInput, FormatOptions, run_fmt};

/// Format `.yar` files (same behavior as the `yarrow-fmt` binary).
pub fn run_fmt_command(
    paths: Vec<PathBuf>,
    check: bool,
    stdin: bool,
    max_width: usize,
    sort_requires: bool,
) -> ExitCode {
    run_fmt(
        "yarrow fmt",
        FmtInput {
            options: FormatOptions {
                max_width,
                sort_requires,
            },
            check,
            stdin,
            paths,
        },
    )
}
