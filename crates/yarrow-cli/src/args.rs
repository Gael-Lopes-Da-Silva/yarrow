//! Clap argument definitions for the Yarrow CLI.

use std::ffi::OsString;

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

/// Artifact written by `compile --target object --emit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompileEmitKind {
    /// Relocatable native object (`.o`). Default.
    Object,
    /// Linked host executable.
    Exe,
}

/// Compile / run backend selected with `--target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TargetKind {
    /// Cranelift in-process machine code (`--target jit`).
    Jit,
    /// Native relocatable object / linked executable (default).
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
    ///
    /// Program arguments go after `--` (for example
    /// `yarrow run file.yar -- arg1 arg2`). With the default
    /// `--target object` they are the child process argv. JIT has no
    /// language-level argv API yet; non-empty args are rejected.
    Run {
        /// Source file to compile and run.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Codegen backend (`jit` or `object`). Default: `object`.
        #[arg(long, value_enum, default_value = "object")]
        target: TargetKind,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,

        /// Arguments forwarded to the program (after `--`).
        #[arg(last = true, value_name = "ARGS")]
        program_args: Vec<OsString>,
    },

    /// Check + codegen without running the entry.
    ///
    /// Default `--target object` writes a native artifact; use `--emit object`
    /// (default, `stem.o`) or `--emit exe` (linked host binary, default `stem`).
    /// `--target jit` finalizes JIT code in-process (no file written).
    Compile {
        /// Source file to compile.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Codegen backend (`jit` or `object`). Default: `object`.
        #[arg(long, value_enum, default_value = "object")]
        target: TargetKind,

        /// Artifact for `--target object`: relocatable `object` or linked `exe`.
        /// Ignored for `--target jit`. Default: `object`.
        #[arg(long, value_enum, default_value = "object")]
        emit: CompileEmitKind,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,

        /// Output path for `--target object` (`object` → `<stem>.o`, `exe` → `<stem>`).
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
    ///
    /// Program arguments after `--` are accepted by clap but rejected until
    /// core exposes an interpret argv API.
    Interpret {
        /// Source file to interpret.
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,

        /// Arguments forwarded to the program (after `--`).
        #[arg(last = true, value_name = "ARGS")]
        program_args: Vec<OsString>,
    },

    /// Interactive interpret loop (line-oriented).
    ///
    /// Snippets without a top-level `function` / `require` are wrapped as a
    /// `main` body. Expressions that leave a value print as the entry result.
    /// Exit with `exit`, `quit`, or EOF.
    Repl,

    /// Format Yarrow source to match the style guide.
    ///
    /// Delegates in-process to `yarrow_fmt` (same as the `yarrow-fmt` binary).
    /// Exit: 0 ok / already formatted; 1 would change (`--check`) or format
    /// failure; 2 usage / I/O.
    Fmt {
        /// Exit 1 if any file would change; do not write.
        #[arg(long)]
        check: bool,

        /// Read source from stdin and write formatted text to stdout.
        #[arg(long)]
        stdin: bool,

        /// Soft wrap width in columns (default 100; minimum 20).
        #[arg(long, value_name = "N", default_value_t = 100)]
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

        /// Files or directories (directories recurse for `*.yar`). Required unless `--stdin`.
        #[arg(value_name = "PATH")]
        paths: Vec<std::path::PathBuf>,
    },

    /// Start the Yarrow language server (LSP over stdio).
    ///
    /// Delegates in-process to `yarrow_lsp`. Point editors at `yarrow lsp`.
    Lsp {
        /// Speak LSP over stdin/stdout (default and only transport in v1).
        #[arg(long, default_value_t = true)]
        stdio: bool,

        /// Extra module search path (in addition to global `-L`).
        #[arg(short = 'L', long = "search-path", value_name = "DIR")]
        search_paths: Vec<std::path::PathBuf>,

        /// Top-level entry function name (default `main`).
        #[arg(long, value_name = "NAME", default_value = "main")]
        main: String,

        /// Disable `textDocument/formatting`.
        #[arg(long)]
        no_format: bool,

        /// Disable `textDocument/inlayHint`.
        #[arg(long)]
        no_inlay: bool,

        /// Log level for process messages on stderr.
        #[arg(long, value_enum, default_value = "info")]
        log_level: crate::commands::LspLogLevel,
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
