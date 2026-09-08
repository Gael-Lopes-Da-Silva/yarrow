//! Range / span format API (Stage 15).
//!
//! Full-document [`format_source`](crate::format_source) is the source of truth.
//! [`format_range`] expands the requested byte span to enclosing top-level item
//! boundaries (and to a whole require run when sorting is on; to the whole file
//! when `reorder_layout` is on or hygiene would shift offsets), then returns one
//! contiguous replacement whose text matches the corresponding slice of the
//! full-document format.

use yarrow_core::Span;
use yarrow_core::diagnostics::SourceFile;
use yarrow_core::parser::ast::{Stmt, StmtKind};

use crate::{
    FormatError, FormatIr, FormatOptions, apply_source_hygiene, build_format_ir, format_source,
};

/// Inclusive-exclusive UTF-8 byte range into a source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end: end.max(start),
        }
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }

    pub fn intersects(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// One contiguous replacement produced by [`format_range`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatRangeEdit {
    /// Byte range in the **original** source replaced by [`Self::new_text`].
    /// May be wider than the requested span (top-level expansion).
    pub range: ByteRange,
    /// Formatted replacement; equals the matching slice of a full-document format.
    pub new_text: String,
    /// True when [`Self::range`] is wider than the requested span.
    pub expanded: bool,
}

impl FormatRangeEdit {
    /// Apply this edit to `source`, returning the rewritten buffer.
    pub fn apply(&self, source: &str) -> String {
        let mut out = String::with_capacity(
            source
                .len()
                .saturating_sub(self.range.len())
                .saturating_add(self.new_text.len()),
        );
        out.push_str(&source[..self.range.start]);
        out.push_str(&self.new_text);
        out.push_str(&source[self.range.end..]);
        out
    }

    /// True when applying the edit would leave `source` unchanged.
    pub fn is_noop(&self, source: &str) -> bool {
        source
            .get(self.range.start..self.range.end)
            .is_some_and(|slice| slice == self.new_text)
    }
}

/// Format the region of `source` covering `span`.
///
/// Parses and formats the whole file via [`format_source`], then returns one
/// contiguous edit for the (possibly expanded) region. Parse failures surface
/// as [`FormatError::Parse`]. Binary / `yarrow fmt` do not expose range mode.
pub fn format_range(
    source: &str,
    span: ByteRange,
    options: &FormatOptions,
) -> Result<FormatRangeEdit, FormatError> {
    let formatted = format_source(source, options)?;
    let requested = clamp_byte_range(source, span);

    let cleaned = apply_source_hygiene(source);
    // Hygiene can shift offsets; fall back to a whole-buffer replace so callers
    // never apply a misaligned slice.
    if cleaned.as_str() != source {
        return Ok(FormatRangeEdit {
            range: ByteRange::new(0, source.len()),
            new_text: formatted,
            expanded: true,
        });
    }

    if options.reorder_layout {
        return Ok(FormatRangeEdit {
            range: ByteRange::new(0, source.len()),
            new_text: formatted,
            expanded: requested.start > 0 || requested.end < source.len(),
        });
    }

    let orig_ir = build_format_ir(source, "<input>")?;
    let (cover, expanded) = expand_to_toplevel(&orig_ir, requested, options.sort_requires);

    if cover.is_empty() || formatted == source {
        return Ok(FormatRangeEdit {
            range: cover,
            new_text: source.get(cover.start..cover.end).unwrap_or("").to_string(),
            expanded,
        });
    }

    let fmt_ir = build_format_ir(&formatted, "<input>")?;
    let keys = item_keys_in_cover(&orig_ir, cover);
    let Some(fmt_cover) = cover_for_keys(&fmt_ir, &keys) else {
        return Ok(FormatRangeEdit {
            range: ByteRange::new(0, source.len()),
            new_text: formatted,
            expanded: true,
        });
    };

    let new_text = formatted
        .get(fmt_cover.start..fmt_cover.end)
        .unwrap_or("")
        .to_string();

    Ok(FormatRangeEdit {
        range: cover,
        new_text,
        expanded,
    })
}

fn clamp_byte_range(source: &str, span: ByteRange) -> ByteRange {
    let len = source.len();
    let start = floor_char_boundary(source, span.start.min(len));
    let end = ceil_char_boundary(source, span.end.min(len)).max(start);
    ByteRange::new(start, end)
}

fn floor_char_boundary(source: &str, mut i: usize) -> usize {
    i = i.min(source.len());
    while i > 0 && !source.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(source: &str, mut i: usize) -> usize {
    i = i.min(source.len());
    while i < source.len() && !source.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn spans_intersect(span: Span, range: ByteRange) -> bool {
    span.lo < range.end && range.start < span.hi
}

fn end_line(file: &SourceFile, span: Span) -> usize {
    if span.hi == 0 {
        return span.line;
    }
    file.location(span.hi.saturating_sub(1)).line
}

fn trim_indent(line: &str) -> &str {
    line.trim_start_matches([' ', '\t'])
}

/// Start line (1-based) of the cover for `items[idx]`, including leading
/// own-line comments after the last blank above the item (printer rules).
fn item_cover_start_line(file: &SourceFile, items: &[Stmt], idx: usize) -> usize {
    let stmt = &items[idx];
    let lo = if idx == 0 {
        1
    } else {
        end_line(file, items[idx - 1].span).saturating_add(1)
    };
    let hi = stmt.span.line.max(1);
    if lo == 0 || lo >= hi {
        return hi;
    }

    let mut last_blank: Option<usize> = None;
    let mut comments: Vec<usize> = Vec::new();
    for line in lo..hi {
        if line > file.line_count() {
            break;
        }
        let trimmed = trim_indent(file.line_text(line));
        if trimmed.is_empty() {
            last_blank = Some(line);
            comments.clear();
        } else if trimmed.starts_with('#') {
            comments.push(line);
        } else {
            last_blank = None;
            comments.clear();
        }
    }

    if let Some(blank) = last_blank {
        // Comments after the blank document this item.
        for line in (blank + 1)..hi {
            if trim_indent(file.line_text(line)).starts_with('#') {
                return line;
            }
        }
        return hi;
    }

    if idx > 0 {
        if let Some(&first) = comments.first() {
            return first;
        }
    } else if let Some(&first) = comments.first() {
        // No blank above the first item: treat contiguous comments as part of
        // the item cover so a selection inside the item still gets them.
        return first;
    }

    hi
}

fn item_cover_end_offset(file: &SourceFile, source: &str, stmt: &Stmt) -> usize {
    let line = end_line(file, stmt.span);
    let next_line = line.saturating_add(1);
    if next_line <= file.line_count() + 1 {
        return file.line_start_offset(next_line).min(source.len());
    }
    source.len()
}

fn cover_for_indices(ir: &FormatIr, first: usize, last: usize) -> ByteRange {
    let file = &ir.file;
    let source = file.source.as_str();
    let items = &ir.program.items;
    let start_line = item_cover_start_line(file, items, first);
    let start = file.line_start_offset(start_line).min(source.len());
    let end = item_cover_end_offset(file, source, &items[last]);
    ByteRange::new(start, end.max(start))
}

fn expand_require_runs(items: &[Stmt], indices: &mut Vec<usize>) {
    if indices.is_empty() {
        return;
    }
    let selected = indices.clone();
    let mut i = 0;
    while i < items.len() {
        if matches!(items[i].kind, StmtKind::Require { .. }) {
            let start = i;
            i += 1;
            while i < items.len() && matches!(items[i].kind, StmtKind::Require { .. }) {
                i += 1;
            }
            if selected.iter().any(|&j| j >= start && j < i) {
                for j in start..i {
                    indices.push(j);
                }
            }
        } else {
            i += 1;
        }
    }
    indices.sort_unstable();
    indices.dedup();
}

fn expand_to_toplevel(
    ir: &FormatIr,
    requested: ByteRange,
    sort_requires: bool,
) -> (ByteRange, bool) {
    let items = &ir.program.items;
    let source_len = ir.file.source.len();
    if items.is_empty() {
        let whole = ByteRange::new(0, source_len);
        let expanded = requested.start > 0 || requested.end < source_len;
        return (whole, expanded);
    }

    let mut indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, stmt)| spans_intersect(stmt.span, requested))
        .map(|(i, _)| i)
        .collect();

    if indices.is_empty() {
        if let Some(i) = items.iter().position(|s| s.span.lo >= requested.start) {
            indices.push(i);
        } else {
            indices.push(items.len() - 1);
        }
    }

    if sort_requires {
        expand_require_runs(items, &mut indices);
    }

    indices.sort_unstable();
    indices.dedup();
    let first = *indices.first().expect("indices non-empty");
    let last = *indices.last().expect("indices non-empty");
    let cover = cover_for_indices(ir, first, last);
    let expanded = cover.start < requested.start || cover.end > requested.end;
    (cover, expanded)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ItemKey {
    Require(String),
    Type(String),
    Implement(String),
    Function(String),
    Other(usize),
}

fn item_key(stmt: &Stmt, index: usize) -> ItemKey {
    match &stmt.kind {
        StmtKind::Require { path, .. } => ItemKey::Require(path.clone()),
        StmtKind::Struct(d) => ItemKey::Type(d.name.clone()),
        StmtKind::Enum(d) => ItemKey::Type(d.name.clone()),
        StmtKind::Union(d) => ItemKey::Type(d.name.clone()),
        StmtKind::Error(d) => ItemKey::Type(d.name.clone()),
        StmtKind::Implement(d) => ItemKey::Implement(d.target.clone()),
        StmtKind::Function(f) => ItemKey::Function(f.name.clone()),
        _ => ItemKey::Other(index),
    }
}

fn item_keys_in_cover(ir: &FormatIr, cover: ByteRange) -> Vec<ItemKey> {
    ir.program
        .items
        .iter()
        .enumerate()
        .filter(|(_, stmt)| spans_intersect(stmt.span, cover))
        .map(|(i, stmt)| item_key(stmt, i))
        .collect()
}

fn cover_for_keys(ir: &FormatIr, keys: &[ItemKey]) -> Option<ByteRange> {
    if keys.is_empty() {
        return Some(ByteRange::new(0, 0));
    }
    let items = &ir.program.items;
    let mut indices = Vec::new();
    for key in keys {
        let idx = items.iter().enumerate().find_map(|(i, stmt)| {
            if &item_key(stmt, i) == key {
                Some(i)
            } else {
                None
            }
        })?;
        indices.push(idx);
    }
    indices.sort_unstable();
    indices.dedup();
    let first = *indices.first()?;
    let last = *indices.last()?;
    Some(cover_for_indices(ir, first, last))
}
