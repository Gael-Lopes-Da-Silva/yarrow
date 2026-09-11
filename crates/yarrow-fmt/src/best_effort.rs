//! Selective construct reprint on recovered parse (Stage 18).
//!
//! When [`crate::format_source_best_effort`] sees a partial parse, it still
//! applies source hygiene everywhere, then reprints only contiguous top-level
//! declaration spans that do not overlap diagnostic error regions and that
//! re-parse cleanly on their own (same text as [`crate::format_source`] on that
//! slice). Broken regions stay byte-identical aside from hygiene.

use yarrow_core::Span;
use yarrow_core::diagnostics::DiagnosticBatch;
use yarrow_core::parser::ast::StmtKind;

use crate::ignore::intersects_ignore;
use crate::range::{ByteRange, cover_for_indices, spans_intersect};
use crate::{FormatIr, FormatOptions, apply_source_hygiene, format_source};

/// Reprint safe recovered top-level declarations inside hygiened `cleaned`.
///
/// `ir` must be the recovering parse of `cleaned`. Always returns hygiened
/// text; may be unchanged when no span is trustworthy enough to rewrite.
pub fn selective_reprint(
    cleaned: &str,
    ir: &FormatIr,
    batch: &DiagnosticBatch,
    options: &FormatOptions,
) -> String {
    let error_ranges = error_byte_ranges(batch, cleaned.len());
    if error_ranges.is_empty() {
        // Diagnostics without spans: stay fail-closed (hygiene only).
        return cleaned.to_string();
    }

    let items = &ir.program.items;
    let mut safe: Vec<usize> = Vec::new();
    for (idx, stmt) in items.iter().enumerate() {
        if !is_reprintable_kind(&stmt.kind) {
            continue;
        }
        if !span_in_bounds(stmt.span, cleaned.len()) {
            continue;
        }
        if error_ranges
            .iter()
            .any(|err| spans_intersect(stmt.span, *err))
        {
            continue;
        }
        let cover = cover_for_indices(ir, idx, idx);
        if cover.end > cleaned.len() || cover.start > cover.end {
            continue;
        }
        if error_ranges.iter().any(|err| cover.intersects(*err)) {
            continue;
        }
        if intersects_ignore(cleaned, cover) {
            continue;
        }
        // Overlap with an earlier accepted cover → drop this item (skewed span).
        if safe
            .iter()
            .any(|&prev| cover_for_indices(ir, prev, prev).intersects(cover))
        {
            continue;
        }
        safe.push(idx);
    }

    if safe.is_empty() {
        return cleaned.to_string();
    }

    safe.sort_by_key(|&idx| cover_for_indices(ir, idx, idx).start);

    let runs = group_source_contiguous_runs(ir, cleaned, &safe);
    let mut edits: Vec<(ByteRange, String)> = Vec::new();
    for run in runs {
        let first = *run.first().expect("run non-empty");
        let last = *run.last().expect("run non-empty");
        let cover = cover_for_indices(ir, first, last);
        let Some(slice) = cleaned.get(cover.start..cover.end) else {
            continue;
        };
        // Trust gate: the cover must format as a complete program on its own.
        match format_source(slice, options) {
            Ok(formatted) => edits.push((cover, formatted)),
            Err(_) => {
                // Try each item alone when the combined cover is untrusted.
                for &idx in &run {
                    let one = cover_for_indices(ir, idx, idx);
                    let Some(one_slice) = cleaned.get(one.start..one.end) else {
                        continue;
                    };
                    if let Ok(text) = format_source(one_slice, options) {
                        push_non_overlapping(&mut edits, one, text);
                    }
                }
            }
        }
    }

    if edits.is_empty() {
        return cleaned.to_string();
    }

    edits.sort_by_key(|(range, _)| range.start);
    let mut out = cleaned.to_string();
    for (range, text) in edits.into_iter().rev() {
        if range.end > out.len() || range.start > range.end {
            continue;
        }
        out.replace_range(range.start..range.end, &text);
    }
    apply_source_hygiene(&out)
}

fn push_non_overlapping(edits: &mut Vec<(ByteRange, String)>, range: ByteRange, text: String) {
    if edits.iter().any(|(prev, _)| prev.intersects(range)) {
        return;
    }
    edits.push((range, text));
}

fn is_reprintable_kind(kind: &StmtKind) -> bool {
    matches!(
        kind,
        StmtKind::Require { .. }
            | StmtKind::Function(_)
            | StmtKind::Struct(_)
            | StmtKind::Enum(_)
            | StmtKind::Union(_)
            | StmtKind::Error(_)
            | StmtKind::Implement(_)
    )
}

fn span_in_bounds(span: Span, len: usize) -> bool {
    span.lo <= span.hi && span.hi <= len
}

fn error_byte_ranges(batch: &DiagnosticBatch, source_len: usize) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    for diag in batch.iter() {
        let Some(span) = diag.primary_span() else {
            continue;
        };
        if span.hi == 0 && span.lo == 0 {
            continue;
        }
        let start = span.lo.min(source_len);
        let end = span.hi.min(source_len).max(start);
        // Point spans still block overlapping items.
        let end = if end == start {
            (start + 1).min(source_len).max(start)
        } else {
            end
        };
        ranges.push(ByteRange::new(start, end));
    }
    ranges
}

/// Group safe indices into runs whose covers are separated only by blank lines
/// or own-line comments (no code that recovery omitted).
fn group_source_contiguous_runs(ir: &FormatIr, source: &str, safe: &[usize]) -> Vec<Vec<usize>> {
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for &idx in safe {
        let cover = cover_for_indices(ir, idx, idx);
        if let Some(run) = runs.last_mut() {
            let prev = *run.last().expect("run non-empty");
            let prev_cover = cover_for_indices(ir, prev, prev);
            if is_inert_gap(source, prev_cover.end, cover.start) {
                run.push(idx);
                continue;
            }
        }
        runs.push(vec![idx]);
    }
    runs
}

fn is_inert_gap(source: &str, from: usize, to: usize) -> bool {
    if from > to || to > source.len() {
        return false;
    }
    if from == to {
        return true;
    }
    for line in source[from..to].split('\n') {
        let trimmed = line
            .trim_start_matches([' ', '\t'])
            .trim_end_matches([' ', '\t']);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return false;
    }
    true
}
