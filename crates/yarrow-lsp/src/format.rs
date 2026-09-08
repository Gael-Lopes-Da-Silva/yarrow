//! Document and range formatting via `yarrow_fmt`.

use tower_lsp_server::ls_types::{Position, Range, TextEdit};
use yarrow_fmt::{ByteRange, FormatOptions, format_range, format_source};

use crate::position::{PositionEncoding, PositionMap};

/// Format `text` as a full-document replace, or `None` when unparseable.
///
/// On parse failure returns `None` so the editor buffer is left intact (no partial rewrite).
/// Unchanged buffers return an empty edit list.
pub fn format_document(text: &str, encoding: PositionEncoding) -> Option<Vec<TextEdit>> {
    let options = FormatOptions::default();
    let formatted = match format_source(text, &options) {
        Ok(s) => s,
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
