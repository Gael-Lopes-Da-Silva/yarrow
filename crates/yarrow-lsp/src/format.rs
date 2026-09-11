//! Document / range / on-type formatting via `yarrow_fmt` and local indent rules.

use tower_lsp_server::ls_types::{Position, Range, TextEdit};
use yarrow_fmt::{ByteRange, FormatOptions, format_range, format_source_best_effort};

use crate::position::{PositionEncoding, PositionMap};

/// Format `text` as a full-document replace, or `None` when unchanged / unreadable.
///
/// Uses [`format_source_best_effort`]: a clean parse gets full layout; an
/// incomplete parse gets source hygiene only (broken regions left intact).
pub fn format_document(text: &str, encoding: PositionEncoding) -> Option<Vec<TextEdit>> {
    let options = FormatOptions::default();
    let formatted = match format_source_best_effort(text, &options) {
        Ok(out) => out.text,
        Err(_) => return None,
    };

    if formatted == text {
        return Some(Vec::new());
    }

    let map = PositionMap::new(text, encoding);
    let end = map.position(text.len());
    let range = Range::new(Position::new(0, 0), end);
    Some(vec![TextEdit::new(range, formatted)])
}

/// Format the region covering `range`, expanding to enclosing top-level items via
/// [`yarrow_fmt::format_range`].
///
/// Returns `None` on parse / format failure (buffer unchanged). Empty edits when
/// the expanded cover is already formatted.
pub fn format_document_range(
    text: &str,
    range: Range,
    encoding: PositionEncoding,
) -> Option<Vec<TextEdit>> {
    let map = PositionMap::new(text, encoding);
    let start = map.offset(range.start).unwrap_or(0);
    let end = map.offset(range.end).unwrap_or(start).max(start);
    let options = FormatOptions::default();
    let edit = match format_range(text, ByteRange::new(start, end), &options) {
        Ok(e) => e,
        Err(_) => return None,
    };

    if edit.is_noop(text) {
        return Some(Vec::new());
    }

    let lsp_range = Range::new(map.position(edit.range.start), map.position(edit.range.end));
    Some(vec![TextEdit::new(lsp_range, edit.new_text)])
}

/// Mechanical on-type indent after `\n` when the previous line opens with `end`.
///
/// Does **not** call [`format_range`] (top-level expansion is too aggressive for a
/// keystroke). Only rewrites leading whitespace on the new line so it matches the
/// `end` line's tab depth. Mid-line / mid-identifier positions and other triggers
/// yield `None` (buffer unchanged). Already-correct indent yields empty edits.
pub fn format_on_type(
    text: &str,
    position: Position,
    ch: &str,
    encoding: PositionEncoding,
) -> Option<Vec<TextEdit>> {
    if ch != "\n" {
        return None;
    }

    let map = PositionMap::new(text, encoding);
    let offset = map.offset(position)?;
    // Cursor sits on the new line (after the typed newline), possibly after
    // client-inserted indent.
    let newline_at = text[..offset].rfind('\n')?;
    let prev_line_start = text[..newline_at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let prev_line = &text[prev_line_start..newline_at];
    if !previous_line_is_end(prev_line) {
        return None;
    }

    let desired_tabs = leading_tabs(prev_line);
    let cur_line_start = newline_at + 1;
    let cur_line_end = text[cur_line_start..]
        .find('\n')
        .map(|i| cur_line_start + i)
        .unwrap_or(text.len());
    let cur_line = &text[cur_line_start..cur_line_end];

    // Skip mid-token: non-indent content already on this line past the cursor,
    // or cursor not within the leading indent run.
    let indent_end = cur_line_start + leading_ws_len(cur_line);
    if offset > indent_end {
        return None;
    }
    if cur_line[leading_ws_len(cur_line)..]
        .chars()
        .any(|c| !c.is_whitespace())
    {
        // Non-ws already typed on the new line: leave it alone.
        return None;
    }

    let desired = "\t".repeat(desired_tabs);
    let current_indent = &text[cur_line_start..indent_end];
    if current_indent == desired {
        return Some(Vec::new());
    }

    let lsp_range = Range::new(map.position(cur_line_start), map.position(indent_end));
    Some(vec![TextEdit::new(lsp_range, desired)])
}

/// True when `line`'s first non-ws word is the keyword `end` (covers `end with T`).
fn previous_line_is_end(line: &str) -> bool {
    let trimmed = line.trim_start_matches([' ', '\t']);
    trimmed == "end" || trimmed.starts_with("end ") || trimmed.starts_with("end\t")
}

fn leading_tabs(line: &str) -> usize {
    line.chars().take_while(|&c| c == '\t').count()
}

fn leading_ws_len(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == '\t' || *c == ' ')
        .map(|c| c.len_utf8())
        .sum()
}
