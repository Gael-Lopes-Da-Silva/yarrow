//! `yarrow-fmt` binary: in-place format, `--check`, and `--stdin`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use yarrow_fmt::{FmtInput, FormatOptions, run_fmt};

/// Exit: 0 ok / already formatted; 1 would change (`--check`) or format failure; 2 usage / I/O.
#[derive(Debug, Parser)]
#[command(
    name = "yarrow-fmt",
    about = "Format Yarrow (.yar) source to match the style guide",
    version
)]
struct Args {
    /// Exit 1 if any file would change; do not write.
    #[arg(long)]
    check: bool,

    /// Read source from stdin and write formatted text to stdout.
    #[arg(long)]
    stdin: bool,

    /// Soft wrap width in columns (default 100).
    #[arg(long, value_name = "N", default_value_t = 100)]
    max_width: usize,

    /// Sort top-level requires (std first, then local; alphabetical within groups).
    #[arg(long)]
    sort_requires: bool,

    /// Reorder top-level items to style-guide file layout (high churn; opt-in).
    #[arg(long)]
    reorder_layout: bool,

    /// Files or directories (directories recurse for `*.yar`). Required unless `--stdin`.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    run_fmt(
        "yarrow-fmt",
        FmtInput {
            options: FormatOptions {
                max_width: args.max_width,
                sort_requires: args.sort_requires,
                reorder_layout: args.reorder_layout,
            },
            check: args.check,
            stdin: args.stdin,
            paths: args.paths,
        },
    )
}
