//! Compile-output hygiene: manifest sidecar for `yarrow clean`.
//!
//! Default `compile` paths stay in the cwd (`<stem>.o` / `<stem>`). Successful
//! writes are recorded under `.yarrow-build/artifacts`. `clean` deletes only
//! those listed paths (never a recursive `*.o` glob). Pre-existing cwd objects
//! that were never recorded are left alone.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::args::GlobalArgs;

/// Directory holding the CLI artifact manifest (cwd-relative).
pub const BUILD_DIR: &str = ".yarrow-build";

/// Manifest file: one UTF-8 path per line (paths as written by `compile`).
pub const MANIFEST_NAME: &str = "artifacts";

fn manifest_path() -> PathBuf {
    PathBuf::from(BUILD_DIR).join(MANIFEST_NAME)
}

/// Record a path produced by this CLI's `compile` so `clean` may delete it.
pub fn record(path: &Path) -> io::Result<()> {
    let entry = path_to_line(path);
    if entry.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(BUILD_DIR)?;
    let mut paths = load_paths()?;
    if paths.insert(entry) {
        write_paths(&paths)?;
    }
    Ok(())
}

/// Remove only paths listed in the manifest; missing files / missing manifest
/// are success. Does not glob the tree.
pub fn clean(global: &GlobalArgs) -> ExitCode {
    let manifest = manifest_path();
    if !manifest.exists() {
        if global.progress() {
            eprintln!("clean: no artifact manifest ({})", manifest.display());
        }
        return ExitCode::SUCCESS;
    }

    let paths = match load_paths() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", manifest.display());
            return ExitCode::from(2);
        }
    };

    for line in &paths {
        let p = Path::new(line);
        match fs::remove_file(p) {
            Ok(()) => {
                if !global.quiet {
                    eprintln!("removed {}", p.display());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!("error: cannot remove {}: {e}", p.display());
                return ExitCode::from(2);
            }
        }
    }

    if let Err(e) = fs::remove_file(&manifest)
        && e.kind() != io::ErrorKind::NotFound
    {
        eprintln!("error: cannot remove {}: {e}", manifest.display());
        return ExitCode::from(2);
    }

    match fs::remove_dir(BUILD_DIR) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => {
            // User or tooling left other files; leave the dir.
            if global.progress() {
                eprintln!("clean: left non-empty {BUILD_DIR}/");
            }
        }
        Err(e) => {
            eprintln!("error: cannot remove {BUILD_DIR}/: {e}");
            return ExitCode::from(2);
        }
    }

    ExitCode::SUCCESS
}

fn path_to_line(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn load_paths() -> io::Result<BTreeSet<String>> {
    let manifest = manifest_path();
    if !manifest.exists() {
        return Ok(BTreeSet::new());
    }
    let text = fs::read_to_string(&manifest)?;
    let mut paths = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        paths.insert(line.to_string());
    }
    Ok(paths)
}

fn write_paths(paths: &BTreeSet<String>) -> io::Result<()> {
    let mut body = String::from("# yarrow compile artifacts (managed by the CLI)\n");
    for p in paths {
        body.push_str(p);
        body.push('\n');
    }
    fs::write(manifest_path(), body)
}
