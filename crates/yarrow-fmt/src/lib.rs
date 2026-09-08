//! Yarrow source formatter.
//!
//! Rewrites `.yar` to match `docs/STYLE_GUIDE.md`. Parses via `yarrow_core`;
//! does not type-check, borrow-check, or codegen.
//!
//! Stage 6: construct layout (requires, types, functions, variables,
//! containers, short calls), then indent and blank-line passes.

mod blank;
mod hygiene;
mod indent;
mod ir;
mod print;

pub use blank::apply_blank_lines;
pub use hygiene::apply_source_hygiene;
pub use indent::apply_indent;
pub use ir::{AttachedComment, Comment, CommentAttach, FormatIr, TriviaMap};
pub use print::apply_construct_layout;

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
/// Stage 6: hygiene, construct layout reprint, tab indent / `end` alignment,
/// then blank-line rules. Parse failures surface as [`FormatError::Parse`].
pub fn format_source(source: &str, options: &FormatOptions) -> Result<String, FormatError> {
    format_source_at(source, "<input>", options)
}

fn format_source_at(
    source: &str,
    path: &str,
    options: &FormatOptions,
) -> Result<String, FormatError> {
    let cleaned = apply_source_hygiene(source);
    let ir = build_format_ir(&cleaned, path)?;
    let laid_out = apply_source_hygiene(&apply_construct_layout(&ir, options));
    // Re-parse so indent / blank line numbers match post-layout text.
    let ir = build_format_ir(&laid_out, path)?;
    let indented = apply_source_hygiene(&apply_indent(&ir));
    let ir = build_format_ir(&indented, path)?;
    let blanked = apply_blank_lines(&ir);
    Ok(apply_source_hygiene(&blanked))
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
    format_source_at(source, &path_str, options)
}
