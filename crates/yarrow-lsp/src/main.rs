//! Entry for `cargo run -p yarrow_lsp` (stdio or TCP).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use yarrow_lsp::LspConfig;

#[derive(Debug, Parser)]
#[command(
    name = "yarrow-lsp",
    about = "Yarrow language server (LSP over stdio or TCP)",
    version
)]
struct Args {
    /// Speak LSP over stdin/stdout (default when `--listen` is omitted).
    #[arg(long, default_value_t = true)]
    stdio: bool,

    /// Accept one TCP client at HOST:PORT (port `0` = ephemeral).
    #[arg(long, value_name = "HOST:PORT")]
    listen: Option<String>,

    /// Extra module search path (repeatable; `-L` equivalent).
    #[arg(short = 'L', long = "search-path", value_name = "DIR")]
    search_paths: Vec<PathBuf>,

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

    let config = LspConfig {
        search_paths: args.search_paths,
        project_roots: Vec::new(),
        entry_name: args.main,
        format_enable: !args.no_format,
        inlay_hints_enable: !args.no_inlay,
    };

    if let Some(addr) = args.listen.as_deref() {
        log_stderr(
            LogLevel::Info,
            args.log_level,
            &format!(
                "starting (tcp listen={addr}; entry={}; format={}; inlay={}; search_paths={})",
                config.entry_name,
                config.format_enable,
                config.inlay_hints_enable,
                config.search_paths.len()
            ),
        );

        // Always print the bound address so harnesses can connect when port is 0.
        let result = yarrow_lsp::run_tcp_with(addr, config, |local| {
            eprintln!("yarrow-lsp: listening on {local}");
        })
        .await;
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                log_stderr(LogLevel::Error, args.log_level, &err.to_string());
                ExitCode::FAILURE
            }
        };
    }

    if !args.stdio {
        eprintln!("yarrow-lsp: pass --stdio (default) or --listen HOST:PORT");
        return ExitCode::from(2);
    }

    log_stderr(
        LogLevel::Info,
        args.log_level,
        &format!(
            "starting (stdio; entry={}; format={}; inlay={}; search_paths={})",
            config.entry_name,
            config.format_enable,
            config.inlay_hints_enable,
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
