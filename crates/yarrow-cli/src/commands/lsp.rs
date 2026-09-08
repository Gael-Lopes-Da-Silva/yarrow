//! `yarrow lsp` - thin in-process wrapper around `yarrow_lsp::run_stdio_blocking`.

use std::path::PathBuf;
use std::process::ExitCode;

use yarrow_lsp::LspConfig;

use crate::args::GlobalArgs;

/// Start the language server on stdio.
pub fn run_lsp(
    global: &GlobalArgs,
    search_paths: &[PathBuf],
    entry_name: &str,
    format_enable: bool,
    inlay_hints_enable: bool,
    log_level: LspLogLevel,
) -> ExitCode {
    let mut search = global.search_paths.clone();
    for p in search_paths {
        if !search.iter().any(|e| e == p) {
            search.push(p.clone());
        }
    }
    let config = LspConfig {
        search_paths: search,
        entry_name: entry_name.to_string(),
        format_enable,
        inlay_hints_enable,
    };

    if log_level.allows(LspLogLevel::Info) && !global.quiet {
        eprintln!(
            "yarrow lsp: starting (stdio; entry={}; format={}; inlay={}; search_paths={})",
            config.entry_name,
            config.format_enable,
            config.inlay_hints_enable,
            config.search_paths.len()
        );
    }

    match yarrow_lsp::run_stdio_blocking(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if log_level.allows(LspLogLevel::Error) {
                eprintln!("yarrow lsp: {err}");
            }
            ExitCode::FAILURE
        }
    }
}

/// stderr log filter for `yarrow lsp --log-level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LspLogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
}

impl LspLogLevel {
    pub fn allows(self, level: LspLogLevel) -> bool {
        match self {
            LspLogLevel::Off => false,
            LspLogLevel::Error => matches!(level, LspLogLevel::Error),
            LspLogLevel::Warn => matches!(level, LspLogLevel::Error | LspLogLevel::Warn),
            LspLogLevel::Info => {
                matches!(
                    level,
                    LspLogLevel::Error | LspLogLevel::Warn | LspLogLevel::Info
                )
            }
            LspLogLevel::Debug => true,
        }
    }
}
