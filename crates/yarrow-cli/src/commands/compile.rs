//! Implementation of the `compile` subcommand.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use yarrow_core::{CompileOptions, ExecutionMode, Session};

use crate::args::{CompileEmitKind, GlobalArgs, TargetKind};
use crate::diagnostics::render_batch;

/// Check + codegen `file` without running the entry.
pub fn compile_file(
    file: &Path,
    target: TargetKind,
    emit: CompileEmitKind,
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
            if emit == CompileEmitKind::Exe {
                eprintln!("error: --emit exe requires --target object");
                return ExitCode::from(2);
            }
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
        TargetKind::Object => match emit {
            CompileEmitKind::Object => {
                opts.mode = ExecutionMode::Object;
                let out = output
                    .map(PathBuf::from)
                    .unwrap_or_else(|| default_object_path(file));
                if global.verbose {
                    eprintln!(
                        "compiling {path} (target object, emit object) -> {}",
                        out.to_string_lossy()
                    );
                }
                let session = Session::new(opts);
                match session.compile_object_source(source) {
                    Ok(artifact) => write_bytes(&out, &artifact.bytes, false, global),
                    Err(diags) => {
                        eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                        ExitCode::from(1)
                    }
                }
            }
            CompileEmitKind::Exe => {
                opts.mode = ExecutionMode::Object;
                let out = output
                    .map(PathBuf::from)
                    .unwrap_or_else(|| default_exe_path(file));
                if global.verbose {
                    eprintln!(
                        "compiling {path} (target object, emit exe) -> {}",
                        out.to_string_lossy()
                    );
                }
                let session = Session::new(opts);
                match session.compile_executable_source(source) {
                    Ok(artifact) => write_bytes(&out, &artifact.bytes, true, global),
                    Err(diags) => {
                        eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                        ExitCode::from(1)
                    }
                }
            }
        },
    }
}

fn write_bytes(out: &Path, bytes: &[u8], executable: bool, global: &GlobalArgs) -> ExitCode {
    if let Err(e) = fs::write(out, bytes) {
        eprintln!("error: cannot write {}: {e}", out.to_string_lossy());
        return ExitCode::from(2);
    }
    if executable {
        if let Err(e) = set_executable(out) {
            eprintln!(
                "error: cannot set execute permission on {}: {e}",
                out.to_string_lossy()
            );
            return ExitCode::from(2);
        }
    }
    if !global.quiet {
        eprintln!("wrote {}", out.to_string_lossy());
    }
    ExitCode::SUCCESS
}

fn set_executable(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn default_object_path(file: &Path) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "a".to_string());
    PathBuf::from(format!("{stem}.o"))
}

fn default_exe_path(file: &Path) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "a".to_string());
    PathBuf::from(stem)
}
