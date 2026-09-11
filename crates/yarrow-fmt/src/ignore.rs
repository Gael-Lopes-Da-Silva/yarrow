//! Diff-friendly ignore regions (`docs/STYLE_GUIDE.md`, Stage 19).
//!
//! Paired whole-line comments `# yarrow-fmt-ignore-begin` /
//! `# yarrow-fmt-ignore-end` mark a contiguous region. Construct layout,
//! indent, blanks, require-sort, and reorder still run on the file; after the
//! full pipeline, [`restore_ignore_regions`] copies each region's original
//! (hygiened) text back so ignored interiors stay byte-stable aside from
//! source hygiene. Unmatched `begin` runs through EOF; stray `end` is ignored.
//! Nested begins are not supported (inner markers stay literal text).

use crate::ByteRange;

const BEGIN: &str = "yarrow-fmt-ignore-begin";
const END: &str = "yarrow-fmt-ignore-end";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Begin,
    End,
}

/// True when `line` (indent-trimmed) is an ignore directive comment.
fn marker_kind(line: &str) -> Option<Marker> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let rest = trimmed.strip_prefix('#')?;
    let rest = rest.trim_start_matches([' ', '\t']);
    if rest == BEGIN
        || rest
            .strip_prefix(BEGIN)
            .is_some_and(|s| s.starts_with([' ', '\t']))
    {
        Some(Marker::Begin)
    } else if rest == END
        || rest
            .strip_prefix(END)
            .is_some_and(|s| s.starts_with([' ', '\t']))
    {
        Some(Marker::End)
    } else {
        None
    }
}

/// Byte ranges (inclusive of marker lines) for each paired ignore region.
///
/// Ranges are ordered by start offset. A `begin` without a matching `end`
/// covers through EOF. A stray `end` does not open a region.
pub fn find_ignore_regions(source: &str) -> Vec<ByteRange> {
    let mut regions = Vec::new();
    let mut pending_start: Option<usize> = None;
    let mut offset = 0usize;
    let bytes = source.as_bytes();

    while offset <= bytes.len() {
        let line_start = offset;
        let line_end = match bytes[offset..].iter().position(|&b| b == b'\n') {
            Some(rel) => offset + rel,
            None => bytes.len(),
        };
        let line = std::str::from_utf8(&bytes[line_start..line_end]).unwrap_or("");
        // Include the newline in the region end when present.
        let after_line = if line_end < bytes.len() {
            line_end + 1
        } else {
            line_end
        };

        match marker_kind(line) {
            Some(Marker::Begin) if pending_start.is_none() => {
                pending_start = Some(line_start);
            }
            Some(Marker::End) => {
                if let Some(start) = pending_start.take() {
                    regions.push(ByteRange::new(start, after_line));
                }
            }
            _ => {}
        }

        if line_end >= bytes.len() {
            break;
        }
        offset = after_line;
    }

    if let Some(start) = pending_start {
        regions.push(ByteRange::new(start, source.len()));
    }

    regions
}

/// True when the file contains at least one ignore begin marker.
pub fn has_ignore_markers(source: &str) -> bool {
    source
        .lines()
        .any(|line| marker_kind(line) == Some(Marker::Begin))
}

/// True when `range` intersects any ignore region.
pub fn intersects_ignore(source: &str, range: ByteRange) -> bool {
    find_ignore_regions(source)
        .iter()
        .any(|r| r.intersects(range))
}

/// Replace formatted ignore interiors with the matching original regions.
///
/// Pairs regions by order. When counts differ (markers lost or invented),
/// returns `formatted` unchanged. Applies replacements from the end so
/// offsets stay valid.
pub fn restore_ignore_regions(original: &str, formatted: &str) -> String {
    let orig_regions = find_ignore_regions(original);
    let fmt_regions = find_ignore_regions(formatted);
    if orig_regions.is_empty() {
        return formatted.to_string();
    }
    if orig_regions.len() != fmt_regions.len() {
        return formatted.to_string();
    }

    let mut out = formatted.to_string();
    for (orig, fmt) in orig_regions.into_iter().zip(fmt_regions).rev() {
        if fmt.end > out.len() || fmt.start > fmt.end {
            continue;
        }
        if orig.end > original.len() || orig.start > orig.end {
            continue;
        }
        let Some(slice) = original.get(orig.start..orig.end) else {
            continue;
        };
        out.replace_range(fmt.start..fmt.end, slice);
    }
    out
}

/// Shrink `cover` so it does not include ignore regions that `requested`
/// does not intersect. If an unselected ignore sits inside `cover`, returns
/// `None` so callers can fall back to a whole-file edit.
pub fn trim_cover_around_ignores(
    source: &str,
    cover: ByteRange,
    requested: ByteRange,
) -> Option<ByteRange> {
    let mut start = cover.start;
    let mut end = cover.end;
    for ign in find_ignore_regions(source) {
        if !cover.intersects(ign) {
            continue;
        }
        if requested.intersects(ign) {
            continue;
        }
        // Unselected ignore overlapping the cover: trim if it only touches an
        // edge; otherwise the contiguous edit cannot skip the middle.
        if ign.end <= requested.start {
            start = start.max(ign.end);
            continue;
        }
        if ign.start >= requested.end {
            end = end.min(ign.start);
            continue;
        }
        return None;
    }
    if start > end {
        return Some(ByteRange::new(start, start));
    }
    Some(ByteRange::new(start, end))
}

/// Ignore region that fully contains `span`, if any.
pub fn enclosing_ignore(source: &str, span: ByteRange) -> Option<ByteRange> {
    find_ignore_regions(source)
        .into_iter()
        .find(|r| r.start <= span.start && span.end <= r.end)
}
