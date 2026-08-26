//! Implementation of the `check` subcommand.

use std::path::Path;
use std::process::ExitCode;

use yarrow_core::{CompileOptions, ExecutionMode, Session};

use crate::args::GlobalArgs;
use crate::diagnostics::render_batch;

pub fn check_file(file: &Path, entry_name: &str, global: &GlobalArgs) -> ExitCode {
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
