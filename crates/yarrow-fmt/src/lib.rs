//! Yarrow source formatter.
//!
//! Rewrites `.yar` to match `docs/STYLE_GUIDE.md`. Parses via `yarrow_core`;
//! does not type-check, borrow-check, or codegen.
//!
//! Stage 3: parses into a format IR, then applies source-file hygiene
//! (LF, no trailing whitespace, final newline). Construct reprint lands later.

mod hygiene;
mod ir;

pub use hygiene::apply_source_hygiene;
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
    /// File bytes are not valid UTF-8 (formatter does not rewrite encoding).
    NotUtf8 { path: PathBuf },
    /// Tokenize or parse failure from `yarrow_core` (no fmt-only syntax errors).
    Parse(SessionDiagnostics),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Io { path, source } => {
                write!(f, "I/O error for {}: {source}", path.display())
            }
            FormatError::NotUtf8 { path } => {
                write!(
                    f,
                    "{}: source is not valid UTF-8; formatter requires UTF-8",
                    path.display()
                )
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
            FormatError::NotUtf8 { .. } | FormatError::Parse(_) => None,
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
/// Stage 3: parses and builds IR, then applies source-file hygiene.
/// Parse failures surface as [`FormatError::Parse`].
pub fn format_source(source: &str, options: &FormatOptions) -> Result<String, FormatError> {
    let cleaned = apply_source_hygiene(source);
    let _ir = build_format_ir(&cleaned, "<input>")?;
    let _ = options;
    Ok(cleaned)
}

/// Read `path` and format its contents.
///
/// Rejects non-UTF-8 files with [`FormatError::NotUtf8`] (no lossy rewrite).
pub fn format_file(path: &Path, options: &FormatOptions) -> Result<String, FormatError> {
    let bytes = std::fs::read(path).map_err(|source| FormatError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let source = std::str::from_utf8(&bytes).map_err(|_| FormatError::NotUtf8 {
        path: path.to_path_buf(),
    })?;
    let path_str = path.to_string_lossy();
    let cleaned = apply_source_hygiene(source);
    let _ir = build_format_ir(&cleaned, &path_str)?;
    let _ = options;
    Ok(cleaned)
}
