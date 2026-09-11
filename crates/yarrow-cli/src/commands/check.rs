//! Implementation of the `check` subcommand.

use std::path::Path;
use std::process::ExitCode;

use yarrow_core::{CompileOptions, ExecutionMode, ProjectOptions, Session};

use crate::args::GlobalArgs;
use crate::diagnostics::render_batch;

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
            eprintln!("error: check requires at least one source file");
            ExitCode::from(2)
        }
        [file] => check_file(file, entry_name, global),
        roots => check_project(roots, entry_name, global),
    }
}

fn check_file(file: &Path, entry_name: &str, global: &GlobalArgs) -> ExitCode {
    let path = file.to_string_lossy().into_owned();
    let color = global.color.to_core();

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::from(2);
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
            ExitCode::SUCCESS
        }
        Err(diags) => {
            eprint!("{}", render_batch(&diags.batch, &diags.file, color));
            ExitCode::from(1)
        }
    }
}

fn check_project(roots: &[std::path::PathBuf], entry_name: &str, global: &GlobalArgs) -> ExitCode {
    let color = global.color.to_core();

    let mut opts = match ProjectOptions::from_root_paths(roots) {
        Ok(opts) => opts,
        Err(diags) => {
            eprint!("{}", render_batch(&diags.batch, &diags.file, color));
            return ExitCode::from(1);
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
        Err(diags) => {
            eprint!("{}", render_batch(&diags.batch, &diags.file, color));
            ExitCode::from(1)
        }
    }
}
