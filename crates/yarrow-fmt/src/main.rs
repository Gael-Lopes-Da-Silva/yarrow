//! `yarrow-fmt` binary: in-place format, `--check`, and `--stdin`.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use yarrow_fmt::{
    FormatError, FormatOptions, collect_yar_paths, format_source, load_and_format, write_formatted,
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

    /// Soft wrap width in columns (default 100).
    #[arg(long, value_name = "N", default_value_t = 100)]
    max_width: usize,

    /// Sort top-level requires (std first, then local; alphabetical within groups).
    #[arg(long)]
    sort_requires: bool,

    /// Files or directories (directories recurse for `*.yar`). Required unless `--stdin`.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let options = FormatOptions {
        max_width: args.max_width,
        sort_requires: args.sort_requires,
    };

    if args.stdin {
        if !args.paths.is_empty() {
            eprintln!("yarrow-fmt: --stdin does not take PATH arguments");
            return ExitCode::from(2);
        }
        return run_stdin(&options, args.check);
    }

    if args.paths.is_empty() {
        eprintln!("yarrow-fmt: pass PATH arguments or --stdin");
        return ExitCode::from(2);
    }

    let files = match collect_yar_paths(&args.paths) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("yarrow-fmt: {err}");
            return ExitCode::from(2);
        }
    };

    if files.is_empty() {
        eprintln!("yarrow-fmt: no .yar files found");
        return ExitCode::from(2);
    }

    let mut had_diff = false;
    let mut had_format_err = false;
    let mut had_io_err = false;

    for path in &files {
        match load_and_format(path, &options) {
            Ok((original, formatted)) => {
                if formatted == original {
                    continue;
                }
                if args.check {
                    eprintln!("would reformat {}", path.display());
                    had_diff = true;
                } else if let Err(err) = write_formatted(path, &formatted) {
                    eprintln!("yarrow-fmt: {err}");
                    match err {
                        FormatError::Io { .. } => had_io_err = true,
                        _ => had_format_err = true,
                    }
                }
            }
            Err(err) => {
                eprintln!("yarrow-fmt: {err}");
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

fn run_stdin(options: &FormatOptions, check: bool) -> ExitCode {
    let mut raw = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut raw) {
        eprintln!("yarrow-fmt: failed to read stdin: {err}");
        return ExitCode::from(2);
    }

    let formatted = match format_source(&raw, options) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("yarrow-fmt: {err}");
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
        eprintln!("yarrow-fmt: failed to write stdout: {err}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
