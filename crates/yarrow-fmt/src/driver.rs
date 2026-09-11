//! Shared formatter driver for `yarrow-fmt` and `yarrow fmt`.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use rayon::prelude::*;

use crate::{
    ByteRange, FormatError, FormatOptions, FormatRangeEdit, MIN_MAX_WIDTH, collect_yar_paths,
    format_range, format_source, format_source_best_effort, load_and_format, write_formatted,
};

/// First line of `--range` stdout (Stage 22). Remaining bytes are `new_text`.
pub const RANGE_EDIT_HEADER: &str = "yarrow-fmt-range-v1";

/// Inputs for one formatter invocation.
#[derive(Debug, Clone)]
pub struct FmtInput {
    pub options: FormatOptions,
    /// Exit 1 if any input would change; do not write.
    pub check: bool,
    /// Read stdin and write formatted text to stdout (no PATH args).
    pub stdin: bool,
    /// On parse failure, apply source hygiene instead of exiting with an error.
    pub best_effort: bool,
    /// When set, format only this UTF-8 byte span via [`format_range`] (Stage 22).
    /// Requires `--stdin` or exactly one `.yar` file. Prints a range-edit encoding
    /// to stdout (does not write the file). Incompatible with `--best-effort`.
    pub range: Option<ByteRange>,
    /// Files or directories (directories recurse for `*.yar`).
    pub paths: Vec<PathBuf>,
}

/// Run the formatter.
///
/// Exit: `0` ok / already formatted; `1` would change (`check`) or format
/// failure; `2` usage / I/O. `program` is the CLI name used in error prefixes
/// (for example `yarrow-fmt` or `yarrow fmt`).
///
/// Rejects `--max-width` / `options.max_width` below [`MIN_MAX_WIDTH`] with
/// exit `2`. Library callers of [`crate::format_source`] clamp instead.
///
/// Multi-file runs format independent paths in parallel (Stage 20). Per-file
/// output bytes match sequential formatting; `--check` / stderr messages stay
/// in sorted path order. Stdin and single-file paths stay sequential.
///
/// With [`FmtInput::range`] (Stage 22), formats one span and prints a
/// [`RANGE_EDIT_HEADER`] encoding to stdout (or only checks with `--check`).
/// Editors should keep using LSP `rangeFormatting` rather than this CLI.
pub fn run_fmt(program: &str, input: FmtInput) -> ExitCode {
    if input.options.max_width < MIN_MAX_WIDTH {
        eprintln!(
            "{program}: --max-width must be at least {MIN_MAX_WIDTH} (got {})",
            input.options.max_width
        );
        return ExitCode::from(2);
    }

    if input.range.is_some() {
        return run_range(program, &input);
    }

    if input.stdin {
        if !input.paths.is_empty() {
            eprintln!("{program}: --stdin does not take PATH arguments");
            return ExitCode::from(2);
        }
        return run_stdin(program, &input.options, input.check, input.best_effort);
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

    let outcomes = format_files(&files, &input.options, input.best_effort);
    report_file_outcomes(program, &files, &outcomes, input.check)
}

/// Parse `--range START:END` (inclusive-exclusive UTF-8 byte offsets).
pub fn parse_range_arg(s: &str) -> Result<ByteRange, String> {
    let (start_s, end_s) = s
        .split_once(':')
        .ok_or_else(|| format!("invalid --range {s:?}; expected START:END byte offsets"))?;
    let start: usize = start_s.parse().map_err(|_| {
        format!("invalid --range start {start_s:?}; expected a non-negative integer")
    })?;
    let end: usize = end_s
        .parse()
        .map_err(|_| format!("invalid --range end {end_s:?}; expected a non-negative integer"))?;
    if end < start {
        return Err(format!(
            "invalid --range {s:?}; END ({end}) must be >= START ({start})"
        ));
    }
    Ok(ByteRange::new(start, end))
}

/// Encode a [`FormatRangeEdit`] for CLI stdout (Stage 22).
///
/// Layout:
/// ```text
/// yarrow-fmt-range-v1
/// <start> <end> <expanded:0|1>
/// <new_text…>
/// ```
pub fn encode_range_edit(edit: &FormatRangeEdit) -> String {
    let expanded = if edit.expanded { 1 } else { 0 };
    format!(
        "{RANGE_EDIT_HEADER}\n{} {} {expanded}\n{}",
        edit.range.start, edit.range.end, edit.new_text
    )
}

fn run_range(program: &str, input: &FmtInput) -> ExitCode {
    let span = input.range.expect("run_range requires range");

    if input.best_effort {
        eprintln!("{program}: --range cannot be combined with --best-effort");
        return ExitCode::from(2);
    }

    let (source, label) = if input.stdin {
        if !input.paths.is_empty() {
            eprintln!("{program}: --stdin does not take PATH arguments");
            return ExitCode::from(2);
        }
        let mut raw = String::new();
        if let Err(err) = io::stdin().read_to_string(&mut raw) {
            eprintln!("{program}: failed to read stdin: {err}");
            return ExitCode::from(2);
        }
        (raw, "<stdin>".to_string())
    } else {
        if input.paths.is_empty() {
            eprintln!("{program}: --range requires --stdin or a single PATH");
            return ExitCode::from(2);
        }
        let files = match collect_yar_paths(&input.paths) {
            Ok(files) => files,
            Err(err) => {
                eprintln!("{program}: {err}");
                return ExitCode::from(2);
            }
        };
        if files.len() != 1 {
            eprintln!(
                "{program}: --range requires exactly one .yar file (found {})",
                files.len()
            );
            return ExitCode::from(2);
        }
        let path = &files[0];
        let label = path.display().to_string();
        match std::fs::read(path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => (s, label),
                Err(_) => {
                    eprintln!(
                        "{program}: {label}: source is not valid UTF-8; formatter requires UTF-8"
                    );
                    return ExitCode::from(2);
                }
            },
            Err(err) => {
                eprintln!("{program}: I/O error for {label}: {err}");
                return ExitCode::from(2);
            }
        }
    };

    let edit = match format_range(&source, span, &input.options) {
        Ok(edit) => edit,
        Err(err) => {
            eprintln!("{program}: {err}");
            // Never half-write: range mode only prints to stdout after success.
            return match err {
                FormatError::Io { .. } | FormatError::NotUtf8 { .. } => ExitCode::from(2),
                FormatError::Parse(_) => ExitCode::from(1),
            };
        }
    };

    if input.check {
        if edit.is_noop(&source) {
            return ExitCode::SUCCESS;
        }
        eprintln!("would reformat range in {label}");
        return ExitCode::from(1);
    }

    let encoded = encode_range_edit(&edit);
    if let Err(err) = io::stdout().write_all(encoded.as_bytes()) {
        eprintln!("{program}: failed to write stdout: {err}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

/// Format each path. Multi-file uses rayon; one file stays sequential.
///
/// Results are aligned with `files` (sorted path order from
/// [`collect_yar_paths`]).
fn format_files(
    files: &[PathBuf],
    options: &FormatOptions,
    best_effort: bool,
) -> Vec<Result<(String, String), FormatError>> {
    if files.len() <= 1 {
        return files
            .iter()
            .map(|path| load_and_format(path, options, best_effort))
            .collect();
    }

    files
        .par_iter()
        .map(|path| load_and_format(path, options, best_effort))
        .collect()
}

fn report_file_outcomes(
    program: &str,
    files: &[PathBuf],
    outcomes: &[Result<(String, String), FormatError>],
    check: bool,
) -> ExitCode {
    let mut had_diff = false;
    let mut had_format_err = false;
    let mut had_io_err = false;

    for (path, result) in files.iter().zip(outcomes.iter()) {
        match result {
            Ok((original, formatted)) => {
                if formatted == original {
                    continue;
                }
                if check {
                    eprintln!("would reformat {}", path.display());
                    had_diff = true;
                } else if let Err(err) = write_formatted(path, formatted) {
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

fn run_stdin(program: &str, options: &FormatOptions, check: bool, best_effort: bool) -> ExitCode {
    let mut raw = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut raw) {
        eprintln!("{program}: failed to read stdin: {err}");
        return ExitCode::from(2);
    }

    let formatted = if best_effort {
        match format_source_best_effort(&raw, options) {
            Ok(out) => out.text,
            Err(err) => {
                eprintln!("{program}: {err}");
                return match err {
                    FormatError::Io { .. } | FormatError::NotUtf8 { .. } => ExitCode::from(2),
                    FormatError::Parse(_) => ExitCode::from(1),
                };
            }
        }
    } else {
        match format_source(&raw, options) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("{program}: {err}");
                return match err {
                    FormatError::Io { .. } | FormatError::NotUtf8 { .. } => ExitCode::from(2),
                    FormatError::Parse(_) => ExitCode::from(1),
                };
            }
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
