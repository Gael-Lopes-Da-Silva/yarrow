//! Shared formatter driver for `yarrow-fmt` and `yarrow fmt` (Stage 12).

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::{
    FormatError, FormatOptions, collect_yar_paths, format_source, load_and_format, write_formatted,
};

/// Inputs for one formatter invocation.
#[derive(Debug, Clone)]
pub struct FmtInput {
    pub options: FormatOptions,
    /// Exit 1 if any input would change; do not write.
    pub check: bool,
    /// Read stdin and write formatted text to stdout (no PATH args).
    pub stdin: bool,
    /// Files or directories (directories recurse for `*.yar`).
    pub paths: Vec<PathBuf>,
}

/// Run the formatter.
///
/// Exit: `0` ok / already formatted; `1` would change (`check`) or format
/// failure; `2` usage / I/O. `program` is the CLI name used in error prefixes
/// (for example `yarrow-fmt` or `yarrow fmt`).
pub fn run_fmt(program: &str, input: FmtInput) -> ExitCode {
    if input.stdin {
        if !input.paths.is_empty() {
            eprintln!("{program}: --stdin does not take PATH arguments");
            return ExitCode::from(2);
        }
        return run_stdin(program, &input.options, input.check);
    }

    if input.paths.is_empty() {
        eprintln!("{program}: pass PATH arguments or --stdin");
        return ExitCode::from(2);
    }

    let files = match collect_yar_paths(&input.paths) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{program}: {err}");
            return ExitCode::from(2);
        }
    };

    if files.is_empty() {
        eprintln!("{program}: no .yar files found");
        return ExitCode::from(2);
    }

    let mut had_diff = false;
    let mut had_format_err = false;
    let mut had_io_err = false;

    for path in &files {
        match load_and_format(path, &input.options) {
            Ok((original, formatted)) => {
                if formatted == original {
                    continue;
                }
                if input.check {
                    eprintln!("would reformat {}", path.display());
                    had_diff = true;
                } else if let Err(err) = write_formatted(path, &formatted) {
                    eprintln!("{program}: {err}");
                    match err {
                        FormatError::Io { .. } => had_io_err = true,
                        _ => had_format_err = true,
                    }
                }
            }
            Err(err) => {
                eprintln!("{program}: {err}");
                match err {
                    FormatError::Io { .. } | FormatError::NotUtf8 { .. } => had_io_err = true,
                    FormatError::Parse(_) => had_format_err = true,
                }
            }
        }
    }

    if had_io_err {
        ExitCode::from(2)
    } else if had_diff || had_format_err {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn run_stdin(program: &str, options: &FormatOptions, check: bool) -> ExitCode {
    let mut raw = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut raw) {
        eprintln!("{program}: failed to read stdin: {err}");
        return ExitCode::from(2);
    }

    let formatted = match format_source(&raw, options) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("{program}: {err}");
            return match err {
                FormatError::Io { .. } | FormatError::NotUtf8 { .. } => ExitCode::from(2),
                FormatError::Parse(_) => ExitCode::from(1),
            };
        }
    };

    if check {
        if formatted != raw {
            eprintln!("would reformat <stdin>");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    if let Err(err) = io::stdout().write_all(formatted.as_bytes()) {
        eprintln!("{program}: failed to write stdout: {err}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
