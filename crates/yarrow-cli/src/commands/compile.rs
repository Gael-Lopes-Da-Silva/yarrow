//! Implementation of the `compile` subcommand.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use yarrow_core::{CompileOptions, ExecutionMode, Session};

use crate::args::{GlobalArgs, TargetKind};
use crate::diagnostics::render_batch;

/// Check + codegen `file` without running the entry.
pub fn compile_file(
    file: &Path,
    target: TargetKind,
    entry_name: &str,
    output: Option<&Path>,
    global: &GlobalArgs,
) -> ExitCode {
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

    match target {
        TargetKind::Jit => {
            opts.mode = ExecutionMode::Jit;
            if global.verbose {
                eprintln!("compiling {path} (target jit)");
            }
            let session = Session::new(opts);
            match session.compile_source(source) {
                Ok(_artifact) => ExitCode::SUCCESS,
                Err(diags) => {
                    eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                    ExitCode::from(1)
                }
            }
        }
        TargetKind::Object => {
            opts.mode = ExecutionMode::Object;
            let out = output
                .map(PathBuf::from)
                .unwrap_or_else(|| default_object_path(file));
            if global.verbose {
                eprintln!(
                    "compiling {path} (target object) -> {}",
                    out.to_string_lossy()
                );
            }
            let session = Session::new(opts);
            match session.compile_object_source(source) {
                Ok(artifact) => {
                    if let Err(e) = std::fs::write(&out, &artifact.bytes) {
                        eprintln!("error: cannot write {}: {e}", out.to_string_lossy());
                        return ExitCode::from(2);
                    }
                    if !global.quiet {
                        eprintln!("wrote {}", out.to_string_lossy());
                    }
                    ExitCode::SUCCESS
                }
                Err(diags) => {
                    eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                    ExitCode::from(1)
                }
            }
        }
    }
}

fn default_object_path(file: &Path) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "a".to_string());
    PathBuf::from(format!("{stem}.o"))
}
