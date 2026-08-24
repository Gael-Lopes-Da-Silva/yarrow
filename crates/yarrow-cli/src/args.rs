//! Clap argument definitions for the Yarrow CLI.

use clap::{Parser, Subcommand};

/// Yarrow language compiler and runner.
#[derive(Debug, Parser)]
#[command(
    name = "yarrow",
    about = "Yarrow language compiler",
    version,
    // `yarrow <file>` is sugar for `yarrow run <file>`.
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    /// Optional default positional. When present and no subcommand is
    /// provided, it is treated as `run <FILE>`.
    #[arg(value_name = "FILE")]
    pub file: Option<std::path::PathBuf>,

    /// Explicit subcommands (use `yarrow run <FILE>`).
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

/// Flags that apply to every subcommand.
#[derive(Debug, Clone, clap::Args)]
pub struct GlobalArgs {
    /// Force color output (always / never / auto).
    #[arg(long, global = true, value_name = "WHEN", default_value = "auto")]
    pub color: ColorArg,

    /// Maximum number of errors to report before stopping.
    #[arg(long, global = true, value_name = "N", default_value_t = yarrow_core::DEFAULT_ERROR_LIMIT)]
    pub error_limit: usize,

    /// Extra module search path (repeatable).
    #[arg(short = 'L', long = "search-path", global = true, value_name = "DIR")]
    pub search_paths: Vec<std::path::PathBuf>,

    /// Suppress non-diagnostic driver output.
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// Enable extra driver progress messages on stderr.
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,
}

/// `--color` argument value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorArg {
    Always,
    Never,
    Auto,
}

impl ColorArg {
    pub fn to_core(self) -> yarrow_core::ColorChoice {
        match self {
            ColorArg::Always => yarrow_core::ColorChoice::Always,
            ColorArg::Never => yarrow_core::ColorChoice::Never,
            ColorArg::Auto => yarrow_core::ColorChoice::Auto,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Compile and run a Yarrow source file.
    ///
    /// This is also the default when you pass a file directly:
    /// `yarrow file.yar` is sugar for `yarrow run file.yar`.
    Run {
        /// Source file to compile and run.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,
    },

    /// Compile a Yarrow source file and report diagnostics, without running `main`.
    Check {
        /// Source file to type-check / validate.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,
    },

    /// Print the CLI version.
    Version,
}
