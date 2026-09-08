//! Yarrow source formatter.
//!
//! Rewrites `.yar` to match `docs/STYLE_GUIDE.md`. Parses via `yarrow_core`;
//! does not type-check, borrow-check, or codegen.
//!
//! Stage 2: `format_source` tokenizes, parses, and builds a format IR
//! (AST + comment trivia). Reprinting lands in later stages; successful
//! format still returns the input unchanged until then.

mod ir;

pub use ir::{AttachedComment, Comment, CommentAttach, FormatIr, TriviaMap};

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use yarrow_core::{ColorChoice, SessionDiagnostics, render_batch};

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
    Io { path: PathBuf, source: io::Error },
    /// Tokenize or parse failure from `yarrow_core` (no fmt-only syntax errors).
    Parse(SessionDiagnostics),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Io { path, source } => {
                write!(f, "I/O error for {}: {source}", path.display())
            }
            FormatError::Parse(diag) => {
                write!(
                    f,
                    "{}",
                    render_batch(&diag.batch, &diag.file, ColorChoice::Never)
                )
            }
        }
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FormatError::Io { source, .. } => Some(source),
            FormatError::Parse(_) => None,
        }
    }
}

impl From<SessionDiagnostics> for FormatError {
    fn from(diag: SessionDiagnostics) -> Self {
        FormatError::Parse(diag)
    }
}

/// Build a format IR from source text (`path` is for diagnostics only).
pub fn build_format_ir(source: &str, path: &str) -> Result<FormatIr, FormatError> {
    FormatIr::parse(source.to_string(), path).map_err(FormatError::from)
}

/// Format a Yarrow source string.
///
/// Stage 2: parses and builds IR; returns `source` unchanged on success.
/// Parse failures surface as [`FormatError::Parse`].
pub fn format_source(source: &str, options: &FormatOptions) -> Result<String, FormatError> {
    let _ir = build_format_ir(source, "<input>")?;
    let _ = options;
    Ok(source.to_string())
}

/// Read `path` and format its contents.
pub fn format_file(path: &Path, options: &FormatOptions) -> Result<String, FormatError> {
    let source = std::fs::read_to_string(path).map_err(|source| FormatError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let path_str = path.to_string_lossy();
    let _ir = build_format_ir(&source, &path_str)?;
    let _ = options;
    Ok(source)
}
