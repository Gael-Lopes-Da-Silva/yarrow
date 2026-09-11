//! Implementation of the `check` subcommand.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use yarrow_core::{CompileOptions, ExecutionMode, ProjectOptions, Session};

use crate::args::GlobalArgs;
use crate::diagnostics::{EXIT_ICE, render_batch, report_session_failure};

/// Check one file (`Session::check_source`) or several roots (`check_project`).
///
/// One path keeps the single-file session. Two or more paths build
/// [`ProjectOptions`] via `from_root_paths` (same product shape as LSP
/// `projectRoots`).
pub fn check_files(
    files: &[std::path::PathBuf],
    entry_name: &str,
    global: &GlobalArgs,
) -> ExitCode {
    match files {
        [] => {
            eprintln!("error: check requires at least one source file or --corpus DIR");
            ExitCode::from(2)
        }
        [file] => check_file(file, entry_name, global),
        roots => check_project(roots, entry_name, global),
    }
}

/// Corpus check: walk `dir` (non-recursive) for `*.yar`, check each with
/// `check_source`, aggregate exit codes.
///
/// Nested directories (for example `helpers/`) are not scanned; those modules
/// are covered via `require` from the top-level programs. Non-`.yar` files are
/// skipped. This is a corpus driver, not a language-level test framework.
pub fn check_corpus(dir: &Path, entry_name: &str, global: &GlobalArgs) -> ExitCode {
    let path = dir.to_string_lossy();

    if !dir.is_dir() {
        eprintln!("error: --corpus expects a directory: {path}");
        return ExitCode::from(2);
    }

    let files = match collect_corpus_yar(dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("error: cannot read corpus {path}: {e}");
            return ExitCode::from(2);
        }
    };

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut saw_ice = false;

    for file in &files {
        if global.progress() {
            eprintln!("checking {}", file.display());
        }
        match check_source_file(file, entry_name, global) {
            CheckOutcome::Ok => ok += 1,
            CheckOutcome::Failed { ice } => {
                failed += 1;
                if ice {
                    saw_ice = true;
                }
            }
            CheckOutcome::Io => return ExitCode::from(2),
        }
    }

    if !global.quiet {
        eprintln!("{ok} ok, {failed} failed");
    }

    if saw_ice {
        ExitCode::from(EXIT_ICE)
    } else if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Immediate `*.yar` children of `dir` (sorted). Subdirectories are ignored.
fn collect_corpus_yar(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yar") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

enum CheckOutcome {
    Ok,
    Failed { ice: bool },
    Io,
}

fn check_file(file: &Path, entry_name: &str, global: &GlobalArgs) -> ExitCode {
    match check_source_file(file, entry_name, global) {
        CheckOutcome::Ok => ExitCode::SUCCESS,
        CheckOutcome::Failed { ice } => {
            if ice {
                ExitCode::from(EXIT_ICE)
            } else {
                ExitCode::from(1)
            }
        }
        CheckOutcome::Io => ExitCode::from(2),
    }
}

fn check_source_file(file: &Path, entry_name: &str, global: &GlobalArgs) -> CheckOutcome {
    let path = file.to_string_lossy().into_owned();
    let color = global.color.to_core();

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return CheckOutcome::Io;
        }
    };

    let mut opts = CompileOptions::new(path);
    for p in &global.search_paths {
        opts.module_search_paths.push(p.clone());
    }
    opts.error_limit = global.error_limit;
    opts.entry_name = entry_name.to_string();
    opts.mode = ExecutionMode::Check;

    let session = Session::new(opts);
    match session.check_source(source) {
        Ok(checked) => {
            if !checked.warnings.is_empty() {
                eprint!("{}", render_batch(&checked.warnings, &checked.file, color));
            }
            CheckOutcome::Ok
        }
        Err(diags) => {
            eprint!("{}", render_batch(&diags.batch, &diags.file, color));
            CheckOutcome::Failed {
                ice: diags.is_ice(),
            }
        }
    }
}

fn check_project(roots: &[std::path::PathBuf], entry_name: &str, global: &GlobalArgs) -> ExitCode {
    let color = global.color.to_core();

    let mut opts = match ProjectOptions::from_root_paths(roots) {
        Ok(opts) => opts,
        Err(diags) => {
            return report_session_failure(&diags, color);
        }
    };
    for p in &global.search_paths {
        opts.module_search_paths.push(p.clone());
    }
    opts.error_limit = global.error_limit;
    opts.entry_name = entry_name.to_string();
    // Match single-file `CompileOptions` / LSP projectRoots defaults.
    opts.require_main = true;

    match Session::check_project(&opts) {
        Ok(checked) => {
            for root in &checked.roots {
                if !root.warnings.is_empty() {
                    eprint!("{}", render_batch(&root.warnings, &root.file, color));
                }
            }
            if global.progress() && !checked.graph.modules.is_empty() {
                eprintln!("project modules: {}", checked.graph.modules.join(", "));
            }
            ExitCode::SUCCESS
        }
        Err(diags) => report_session_failure(&diags, color),
    }
}
