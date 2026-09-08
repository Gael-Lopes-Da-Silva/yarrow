//! Comment spacing from `docs/STYLE_GUIDE.md` (Stage 9).
//!
//! - Own-line and trailing comments: single space after `#`.
//! - Trailing comments: exactly one space before `#`.
//! - Comment body text is otherwise preserved (including interior spaces).

use crate::ir::{Comment, CommentAttach, TriviaMap};

/// Normalize a comment lexeme (`#` … EOL) to `#` or `# <text>`.
///
/// Collapses whitespace immediately after `#`; leaves the remainder unchanged
/// aside from stripping that leading padding.
pub fn normalize_comment(lexeme: &str) -> String {
    let rest = lexeme.strip_prefix('#').unwrap_or(lexeme);
    let rest = rest.trim_start_matches([' ', '\t']);
    if rest.is_empty() {
        String::from("#")
    } else {
        format!("# {rest}")
    }
}

/// Suffix to append on a code line for a trailing comment (` # text`).
pub fn trailing_suffix(lexeme: &str) -> String {
    format!(" {}", normalize_comment(lexeme))
}

/// Trailing comments whose `#` sits on `line` (1-based), in source order.
pub fn trailing_on_line(trivia: &TriviaMap, line: usize) -> Vec<&Comment> {
    let mut out: Vec<&Comment> = trivia
        .all_attached()
        .filter(|a| a.attach == CommentAttach::Trailing && a.comment.span.line == line)
        .map(|a| &a.comment)
        .collect();
    out.sort_by_key(|c| c.span.lo);
    out
}
