//! Implementation of the `run` subcommand.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use yarrow_core::{CompileOptions, ExecutionMode, Session};

use crate::args::{GlobalArgs, TargetKind};
use crate::commands::print_run_result;
use crate::diagnostics::{render_batch, render_diag};

/// Compile and execute `file`, printing any return value from the entry (JIT)
/// or running the linked native binary (`--target object`).
pub fn run_file(
    file: &Path,
    target: TargetKind,
    entry_name: &str,
    global: &GlobalArgs,
) -> ExitCode {
    match target {
        TargetKind::Jit => run_jit(file, entry_name, global),
        TargetKind::Object => run_object(file, entry_name, global),
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
            print_run_result(result);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprint!("{}", render_diag(&e.diagnostic, &artifact.file, color));
            ExitCode::from(1)
        }
    }
}

fn run_object(file: &Path, entry_name: &str, global: &GlobalArgs) -> ExitCode {
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
    opts.mode = ExecutionMode::Object;

    if global.verbose {
        eprintln!("running {path} (target object, entry {entry_name})");
    }

    let session = Session::new(opts);
    let artifact = match session.compile_executable_source(source) {
        Ok(artifact) => artifact,
        Err(diags) => {
            eprint!("{}", render_batch(&diags.batch, &diags.file, color));
            return ExitCode::from(1);
        }
    };

    let exe = match TempExe::write(&artifact.bytes) {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("error: cannot write temporary executable: {e}");
            return ExitCode::from(2);
        }
    };

    if global.verbose {
        eprintln!("exec {}", exe.path.display());
    }

    let status = match Command::new(&exe.path).status() {
        Ok(status) => status,
        Err(e) => {
            eprintln!("error: failed to execute {}: {e}", exe.path.display());
            return ExitCode::from(2);
        }
    };

    match status.code() {
        Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
        Some(_) => ExitCode::from(1),
        None => {
            eprintln!("error: process terminated by signal");
            ExitCode::from(2)
        }
    }
}

/// Temporary host executable removed on drop.
struct TempExe {
    path: PathBuf,
}

impl TempExe {
    fn write(bytes: &[u8]) -> std::io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("yarrow-run-{nanos}-{}", std::process::id()));
        fs::write(&path, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms)?;
        }
        Ok(Self { path })
    }
}

impl Drop for TempExe {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
