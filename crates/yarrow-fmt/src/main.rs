//! `yarrow-fmt` binary: in-place format, `--check`, and `--stdin`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use yarrow_fmt::{
    DEFAULT_MAX_WIDTH, FmtInput, FormatOptions, resolve_sort_requires_flags, run_fmt,
};

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

    /// Soft wrap width in columns (default 100; minimum 20).
    #[arg(long, value_name = "N", default_value_t = DEFAULT_MAX_WIDTH)]
    max_width: usize,

    /// Force-on top-level require sorting (default is already on).
    #[arg(long = "sort-requires", overrides_with = "no_sort_requires")]
    sort_requires: bool,

    /// Keep top-level require source order.
    #[arg(long = "no-sort-requires", overrides_with = "sort_requires")]
    no_sort_requires: bool,

    /// Reorder top-level items to style-guide file layout (high churn; opt-in).
    #[arg(long)]
    reorder_layout: bool,

    /// On parse failure, apply LF / trailing-WS / final-newline hygiene only.
    #[arg(long)]
    best_effort: bool,

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
                sort_requires: resolve_sort_requires_flags(
                    args.sort_requires,
                    args.no_sort_requires,
                ),
                reorder_layout: args.reorder_layout,
            },
            check: args.check,
            stdin: args.stdin,
            best_effort: args.best_effort,
            paths: args.paths,
        },
    )
}
