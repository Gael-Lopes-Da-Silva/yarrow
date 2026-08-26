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

/// Intermediate form printed by `dump --emit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum EmitKind {
    Tokens,
    Ast,
    Ir,
}

/// Compile / run backend selected with `--target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TargetKind {
    /// Cranelift in-process machine code (default).
    Jit,
    /// Native relocatable object (AOT).
    Object,
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

        /// Codegen backend (`jit` or `object`). Default: `jit`.
        #[arg(long, value_enum, default_value = "jit")]
        target: TargetKind,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,
    },

    /// Check + codegen without running the entry.
    ///
    /// Default `--target jit` finalizes JIT code in-process. `--target object`
    /// writes a native relocatable object (`-o`, default `stem.o`).
    Compile {
        /// Source file to compile.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Codegen backend (`jit` or `object`). Default: `jit`.
        #[arg(long, value_enum, default_value = "jit")]
        target: TargetKind,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,

        /// Output path for `--target object` (default: `<stem>.o`).
        #[arg(short = 'o', long = "output", value_name = "PATH")]
        output: Option<std::path::PathBuf>,
    },

    /// Compile a Yarrow source file and report diagnostics, without running `main`.
    Check {
        /// Source file to type-check / validate.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,
    },

    /// Check and interpret a Yarrow source file (no machine code).
    ///
    /// Executes the entry (`main` or `--main`) on the stack VM. There is no
    /// `--target` on this command; use `run` / `compile` for JIT or object.
    Interpret {
        /// Source file to interpret.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,
    },

    /// Print the long form of a diagnostic code.
    Explain {
        /// Diagnostic code, for example `E308`.
        #[arg(value_name = "CODE")]
        code: String,
    },

    /// Print the CLI version.
    Version,

    /// Print an intermediate representation and exit (no run).
    Dump {
        /// Source file to dump.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,
        /// Intermediate to print (`tokens`, `ast`, or `ir`).
        #[arg(long, value_enum, default_value = "ir")]
        emit: EmitKind,
    },
}
