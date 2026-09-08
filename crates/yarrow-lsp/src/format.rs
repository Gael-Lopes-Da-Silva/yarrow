//! Full-document `textDocument/formatting` via `yarrow_fmt::format_source`.

use tower_lsp_server::ls_types::{Position, Range, TextEdit};
use yarrow_fmt::{FormatOptions, format_source};

use crate::position::{PositionEncoding, PositionMap};

/// Format `text` as a full-document replace, or `None` when unchanged / unparseable.
///
/// On parse failure returns `None` so the editor buffer is left intact (no partial rewrite).
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
