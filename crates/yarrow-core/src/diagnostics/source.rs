//! Source file buffer and line-index lookup for span rendering.

use super::Span;
use crate::tokenizer::token::Location;

/// A source file: path, full text, and line start offsets for O(log n) lookup.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub source: String,
    /// Byte offset of the first character of each 1-based line.
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(path: impl Into<String>, source: impl Into<String>) -> Self {
        let source = source.into();
        let mut line_starts = vec![0];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            path: path.into(),
            source,
            line_starts,
        }
    }

    pub fn len(&self) -> usize {
        self.source.len()
    }

    pub fn is_empty(&self) -> bool {
        self.source.is_empty()
    }

    /// 1-based line and column (columns count Unicode scalar values, tabs as 1).
    pub fn location(&self, offset: usize) -> Location {
        let offset = offset.min(self.source.len());
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        let column = self.source[line_start..offset].chars().count() + 1;
        Location::new(line_idx + 1, column, offset)
    }

    pub fn span_start(&self, span: Span) -> Location {
        self.location(span.lo)
    }

    pub fn span_end(&self, span: Span) -> Location {
        self.location(span.hi)
    }

    /// Line text without trailing `\n` / `\r`.
    pub fn line_text(&self, line: usize) -> &str {
        if line == 0 {
            return "";
        }
        let idx = line - 1;
        if idx >= self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[idx];
        let end = if idx + 1 < self.line_starts.len() {
            self.line_starts[idx + 1]
        } else {
            self.source.len()
        };
        let mut text = &self.source[start..end];
        if let Some(stripped) = text.strip_suffix('\n') {
            text = stripped;
        }
        if let Some(stripped) = text.strip_suffix('\r') {
            text = stripped;
        }
        text
    }

    pub fn line_count(&self) -> usize {
        if self.source.is_empty() {
            0
        } else if self.source.ends_with('\n') {
            self.line_starts.len() - 1
        } else {
            self.line_starts.len()
        }
    }

    /// Byte offset of the first character of 1-based `line`, or `source.len()`.
    pub fn line_start_offset(&self, line: usize) -> usize {
        if line == 0 || self.line_starts.is_empty() {
            return 0;
        }
        let idx = (line - 1).min(self.line_starts.len() - 1);
        self.line_starts[idx]
    }
}
