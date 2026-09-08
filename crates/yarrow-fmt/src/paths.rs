//! Collect `.yar` inputs for the formatter driver (Stage 11).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Expand `inputs` into concrete files: directories are walked recursively for
/// `*.yar`; explicit file paths are kept as given (any extension).
pub fn collect_yar_paths(inputs: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for path in inputs {
        if path.is_dir() {
            collect_dir(path, &mut out)?;
        } else {
            out.push(path.clone());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_dir(&path, out)?;
        } else if is_yar_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_yar_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "yar")
}
