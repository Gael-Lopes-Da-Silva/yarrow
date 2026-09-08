//! stdio entry for `cargo run -p yarrow_lsp`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use yarrow_lsp::LspConfig;

#[derive(Debug, Parser)]
#[command(
    name = "yarrow-lsp",
    about = "Yarrow language server (LSP over stdio)",
    version
)]
struct Args {
    /// Speak LSP over stdin/stdout (default and only transport in v1).
    #[arg(long, default_value_t = true)]
    stdio: bool,

    /// Extra module search path (repeatable; `-L` equivalent).
    #[arg(short = 'L', long = "search-path", value_name = "DIR")]
    search_paths: Vec<PathBuf>,

    /// Top-level entry function name (default `main`).
    #[arg(long, value_name = "NAME", default_value = "main")]
    main: String,

    /// Disable `textDocument/formatting`.
    #[arg(long)]
    no_format: bool,

    /// Log level for process messages on stderr.
    #[arg(long, value_enum, default_value = "info")]
    log_level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    fn allows(self, level: LogLevel) -> bool {
        match self {
            LogLevel::Off => false,
            LogLevel::Error => matches!(level, LogLevel::Error),
            LogLevel::Warn => matches!(level, LogLevel::Error | LogLevel::Warn),
            LogLevel::Info => {
                matches!(level, LogLevel::Error | LogLevel::Warn | LogLevel::Info)
            }
            LogLevel::Debug => true,
        }
    }
}

fn log_stderr(level: LogLevel, configured: LogLevel, msg: &str) {
    if configured.allows(level) {
        eprintln!("yarrow-lsp: {msg}");
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    if !args.stdio {
        eprintln!("yarrow-lsp: only --stdio transport is supported");
        return ExitCode::from(2);
    }

    let config = LspConfig {
        search_paths: args.search_paths,
        entry_name: args.main,
        format_enable: !args.no_format,
    };

    log_stderr(
        LogLevel::Info,
        args.log_level,
        &format!(
            "starting (stdio; entry={}; format={}; search_paths={})",
            config.entry_name,
            config.format_enable,
            config.search_paths.len()
        ),
    );

    match yarrow_lsp::run_stdio_with(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            log_stderr(LogLevel::Error, args.log_level, &err.to_string());
            ExitCode::FAILURE
        }
    }
}
