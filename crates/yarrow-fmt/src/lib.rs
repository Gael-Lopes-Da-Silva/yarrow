//! Yarrow source formatter.
//!
//! Rewrites `.yar` to match `docs/STYLE_GUIDE.md`. Parses via `yarrow_core`;
//! does not type-check, borrow-check, or codegen.
//!
//! Stage 16: corpus gate is `docs/examples/valid` and `lib/std` via
//! `scripts/fmt-check.sh` / CI. Shared [`run_fmt`] for `yarrow-fmt` /
//! `yarrow fmt`; [`format_range`] for span edits.

mod blank;
mod comment;
mod driver;
mod hygiene;
mod indent;
mod ir;
mod layout;
mod paths;
mod phrase;
mod print;
mod range;
mod require;

pub use blank::apply_blank_lines;
pub use comment::{normalize_comment, trailing_suffix};
pub use driver::{FmtInput, run_fmt};
pub use hygiene::apply_source_hygiene;
pub use indent::apply_indent;
pub use ir::{AttachedComment, Comment, CommentAttach, FormatIr, TriviaMap};
pub use layout::{LayoutKind, layout_kind, reorder_toplevel_indices, reorder_toplevel_items};
pub use paths::collect_yar_paths;
pub use print::apply_construct_layout;
pub use range::{ByteRange, FormatRangeEdit, format_range};
pub use require::{is_std_path, sort_toplevel_requires};

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use yarrow_core::{ColorChoice, SessionDiagnostics, render_batch};

/// Soft-wrap floor: CLI rejects `--max-width` below this; library formatting
/// clamps to it. Tabs-only indent is fixed (no spaces-indent option).
pub const MIN_MAX_WIDTH: usize = 20;

/// Default soft wrap target in columns.
pub const DEFAULT_MAX_WIDTH: usize = 100;

/// Options controlling layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    /// Soft wrap target in columns. Default: [`DEFAULT_MAX_WIDTH`]. Values
    /// below [`MIN_MAX_WIDTH`] are clamped when formatting.
    pub max_width: usize,
    /// When true, sort consecutive top-level `require` lines: `"std.…"` first,
    /// then other paths, alphabetically within each group. Default true;
    /// disable with `--no-sort-requires`. Function-local requires are never
    /// moved.
    pub sort_requires: bool,
    /// When true, reorder top-level items to style-guide file layout (requires,
    /// types with matching implements, private helpers, public API, `main`).
    /// Opt-in (default false). Function-local items are never moved.
    pub reorder_layout: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            max_width: DEFAULT_MAX_WIDTH,
            sort_requires: true,
            reorder_layout: false,
        }
    }
}

impl FormatOptions {
    /// Soft wrap width after applying [`MIN_MAX_WIDTH`].
    pub fn effective_max_width(&self) -> usize {
        self.max_width.max(MIN_MAX_WIDTH)
    }
}

/// Resolve CLI `--sort-requires` / `--no-sort-requires` (default on).
///
/// Pass the raw clap bools; pair them with `overrides_with` so the last flag
/// wins. When neither is set, returns `true`.
pub fn resolve_sort_requires_flags(sort_requires: bool, no_sort_requires: bool) -> bool {
    match (sort_requires, no_sort_requires) {
        (_, true) => false,
        (true, false) | (false, false) => true,
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
/// Hygiene, construct layout (width wrap, comment spacing, require sort by
/// default, optional file-layout reorder), tab indent / `end` alignment, then
/// blank-line rules. Parse failures surface as [`FormatError::Parse`].
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
    let indented = apply_source_hygiene(&apply_indent(&ir, options));
    let ir = build_format_ir(&indented, path)?;
    let blanked = apply_blank_lines(&ir);
    Ok(apply_source_hygiene(&blanked))
}

/// Read `path` and format its contents.
///
/// Rejects non-UTF-8 files with [`FormatError::NotUtf8`] (no lossy rewrite).
pub fn format_file(path: &Path, options: &FormatOptions) -> Result<String, FormatError> {
    Ok(load_and_format(path, options)?.1)
}

/// Read `path` once and return `(original, formatted)`.
///
/// Rejects non-UTF-8 files with [`FormatError::NotUtf8`] (no lossy rewrite).
pub fn load_and_format(
    path: &Path,
    options: &FormatOptions,
) -> Result<(String, String), FormatError> {
    let bytes = std::fs::read(path).map_err(|source| FormatError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let source = std::str::from_utf8(&bytes).map_err(|_| FormatError::NotUtf8 {
        path: path.to_path_buf(),
    })?;
    let path_str = path.to_string_lossy();
    let formatted = format_source_at(source, &path_str, options)?;
    Ok((source.to_string(), formatted))
}

/// Write UTF-8 `contents` to `path` (creates or replaces the file).
pub fn write_formatted(path: &Path, contents: &str) -> Result<(), FormatError> {
    std::fs::write(path, contents.as_bytes()).map_err(|source| FormatError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// True when formatting `source` would change its bytes.
pub fn would_reformat(source: &str, options: &FormatOptions) -> Result<bool, FormatError> {
    let formatted = format_source(source, options)?;
    Ok(formatted != source)
}
