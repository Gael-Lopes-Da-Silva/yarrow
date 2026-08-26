//! Implementation of the `interpret` subcommand.

use std::path::Path;
use std::process::ExitCode;

use yarrow_core::{CompileOptions, ExecutionMode, Session};

use crate::args::GlobalArgs;
use crate::commands::print_run_result;
use crate::diagnostics::render_batch;

/// Check and interpret `file`, printing any return value from the entry.
pub fn interpret_file(file: &Path, entry_name: &str, global: &GlobalArgs) -> ExitCode {
    let path = file.to_string_lossy().into_owned();
    let color = global.color.to_core();

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };

    let mut opts = CompileOptions::new(path.clone());
    for p in &global.search_paths {
        opts.module_search_paths.push(p.clone());
    }
    opts.error_limit = global.error_limit;
    opts.entry_name = entry_name.to_string();
    opts.mode = ExecutionMode::Interpret;

    if global.verbose {
        eprintln!("interpreting {path} (entry {entry_name})");
    }

    let session = Session::new(opts);
    match session.interpret_source(source) {
        Ok(result) => {
            print_run_result(result);
            ExitCode::SUCCESS
        }
        Err(diags) => {
            eprint!("{}", render_batch(&diags.batch, &diags.file, color));
            ExitCode::from(1)
        }
    }
}
