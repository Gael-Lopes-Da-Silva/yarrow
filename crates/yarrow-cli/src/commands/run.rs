//! Implementation of the `run` subcommand.

use std::path::Path;
use std::process::ExitCode;

use yarrow_core::{CompileOptions, ExecutionMode, RunResult, Session};

use crate::args::{GlobalArgs, TargetKind};
use crate::diagnostics::{render_batch, render_diag};

/// Compile and execute `file`, printing any return value from the entry.
pub fn run_file(
    file: &Path,
    target: TargetKind,
    entry_name: &str,
    global: &GlobalArgs,
) -> ExitCode {
    match target {
        TargetKind::Jit => run_jit(file, entry_name, global),
        TargetKind::Object => {
            // Link + exec lands in Stage 8 (core Stage 19). Do not fall back to JIT.
            let _ = (file, entry_name, global);
            eprintln!(
                "error: `run --target object` is not implemented yet (link + execute)\n\
                 note: use `yarrow compile --target object <file>` to emit a `.o`,\n\
                       or `yarrow run --target jit <file>` (default) to execute in-process"
            );
            ExitCode::from(2)
        }
    }
}

fn run_jit(file: &Path, entry_name: &str, global: &GlobalArgs) -> ExitCode {
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
    opts.mode = ExecutionMode::Jit;

    if global.verbose {
        eprintln!("running {path} (target jit, entry {entry_name})");
    }

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
            eprint!("{}", render_diag(&e.diagnostic, &artifact.file, color));
            ExitCode::from(1)
        }
    }
}
