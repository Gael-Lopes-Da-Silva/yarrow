//! Full-document `textDocument/formatting` via `yarrow_fmt`.

use tower_lsp_server::ls_types::{Position, Range, TextEdit};
use yarrow_fmt::{FormatOptions, format_source_best_effort};

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
