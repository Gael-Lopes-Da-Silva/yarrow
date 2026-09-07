//! Yarrow source formatter.
//!
//! Rewrites `.yar` to match `docs/STYLE_GUIDE.md`. Parses via `yarrow_core`;
//! does not type-check, borrow-check, or codegen.
//!
//! Stage 0: public API stubs. `format_source` currently returns the input unchanged.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

// Path dependency for Stages 1+ (tokenize / parse). Unused in the Stage 0 stub.
use yarrow_core as _;

/// Options controlling layout. More fields land in later stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    /// Soft wrap target in columns. Default: 100.
    pub max_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self { max_width: 100 }
    }
}

/// Failure while formatting a source string or file.
#[derive(Debug)]
pub enum FormatError {
    /// Filesystem read/write failure.
    Io {
        path: PathBuf,
        source: io::Error,
    },
    // Later stages: parse diagnostics from `yarrow_core`.
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Io { path, source } => {
                write!(f, "I/O error for {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FormatError::Io { source, .. } => Some(source),
        }
    }
}

/// Format a Yarrow source string.
///
/// Stage 0 stub: returns `source` unchanged. Later stages parse and reprint.
pub fn format_source(source: &str, _options: &FormatOptions) -> Result<String, FormatError> {
    Ok(source.to_string())
}

/// Read `path` and format its contents.
pub fn format_file(path: &Path, options: &FormatOptions) -> Result<String, FormatError> {
    let source = std::fs::read_to_string(path).map_err(|source| FormatError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    format_source(&source, options)
}
