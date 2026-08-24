//! Implementation of the `run` subcommand.

use std::path::Path;
use std::process::ExitCode;

use yarrow_core::{CompileOptions, RunResult, Session, render, render_batch};

use crate::args::GlobalArgs;

/// Compile and execute `file`, printing any return value from `main`.
pub fn run_file(file: &Path, global: &GlobalArgs) -> ExitCode {
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

    let session = Session::new(opts);
    let mut artifact = match session.compile_source(source) {
        Ok(artifact) => artifact,
        Err(diags) => {
            eprint!("{}", render_batch(&diags.batch, &diags.file, color));
            return ExitCode::from(1);
        }
    };

    match artifact.run_main() {
        Ok(result) => {
            match result {
                RunResult::Void => {}
                RunResult::Int(v) => println!("{v}"),
                RunResult::Bool(b) => println!("{b}"),
                RunResult::Float(f) => println!("{f}"),
                RunResult::Str(s) => println!("{s}"),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            let mut diag = (*e.diagnostic).clone();
            if diag.path.is_empty() {
                diag.path = artifact.file.path.clone();
            }
            eprint!("{}", render(&diag, &artifact.file, color));
            ExitCode::from(1)
        }
    }
}
