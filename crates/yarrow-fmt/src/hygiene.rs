//! Source-file hygiene from `docs/STYLE_GUIDE.md` (Source files).
//!
//! Stage 3: LF endings, no trailing whitespace, exactly one final newline.
//! Encoding (UTF-8) is enforced at the `&str` / file-read boundary.

/// Apply style-guide source-file hygiene to UTF-8 text.
///
/// - Normalizes `\r\n` and lone `\r` to `\n`
/// - Strips trailing ASCII spaces and tabs on every line
/// - Ensures the result ends with exactly one `\n` (empty input → `"\n"`)
///
/// Idempotent: `apply_source_hygiene(apply_source_hygiene(s)) == apply_source_hygiene(s)`.
pub fn apply_source_hygiene(source: &str) -> String {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");

    if normalized.is_empty() {
        return String::from("\n");
    }

    let body = normalized.strip_suffix('\n').unwrap_or(normalized.as_str());
    let mut out = String::with_capacity(body.len() + 1);
    for (i, line) in body.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end_matches([' ', '\t']));
    }
    out.push('\n');
    out
}
