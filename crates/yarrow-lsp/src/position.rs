//! LSP position / range conversion against byte-offset [`yarrow_core::Span`]s.

use tower_lsp_server::ls_types::{Position, PositionEncodingKind, Range};
use yarrow_core::{SourceFile, Span};

/// Negotiated character encoding for LSP `Position.character`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionEncoding {
    /// UTF-8 code units (preferred when the client advertises it).
    Utf8,
    /// UTF-16 code units (LSP default / VS Code-class clients).
    #[default]
    Utf16,
}

impl PositionEncoding {
    pub fn as_lsp(self) -> PositionEncodingKind {
        match self {
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf16 => PositionEncodingKind::UTF16,
        }
    }

    /// Prefer UTF-8 when listed; otherwise UTF-16.
    pub fn negotiate(client_encodings: Option<&[PositionEncodingKind]>) -> Self {
        if let Some(list) = client_encodings
            && list.iter().any(|e| e == &PositionEncodingKind::UTF8)
        {
            return Self::Utf8;
        }
        Self::Utf16
    }
}

/// Maps byte offsets in a source buffer to LSP positions for one encoding.
#[derive(Debug, Clone, Copy)]
pub struct PositionMap<'a> {
    source: &'a str,
    encoding: PositionEncoding,
}

impl<'a> PositionMap<'a> {
    pub fn new(source: &'a str, encoding: PositionEncoding) -> Self {
        Self { source, encoding }
    }

    pub fn from_file(file: &'a SourceFile, encoding: PositionEncoding) -> Self {
        Self::new(&file.source, encoding)
    }

    /// Zero-based LSP position for a byte offset (clamped to EOF).
    pub fn position(&self, offset: usize) -> Position {
        let offset = offset.min(self.source.len());
        let (line, line_start) = line_start_at(self.source, offset);
        let prefix = &self.source[line_start..offset];
        let character = match self.encoding {
            PositionEncoding::Utf8 => prefix.len() as u32,
            PositionEncoding::Utf16 => utf16_len(prefix) as u32,
        };
        Position::new(line, character)
    }

    /// LSP range for an inclusive-exclusive byte [`Span`].
    pub fn range(&self, span: Span) -> Range {
        Range::new(self.position(span.lo), self.position(span.hi))
    }

    /// Byte offset for an LSP position, or `None` if the line is past EOF.
    pub fn offset(&self, pos: Position) -> Option<usize> {
        let line_start = line_start_for_line(self.source, pos.line)?;
        let line_end = match self.source[line_start..].find('\n') {
            Some(rel) => line_start + rel,
            None => self.source.len(),
        };
        let line_text = &self.source[line_start..line_end];
        let within = match self.encoding {
            PositionEncoding::Utf8 => utf8_offset_in_line(line_text, pos.character as usize),
            PositionEncoding::Utf16 => utf16_offset_in_line(line_text, pos.character as usize),
        };
        Some(line_start + within)
    }
}

fn line_start_at(source: &str, offset: usize) -> (u32, usize) {
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (i, b) in source.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    (line, line_start)
}

fn line_start_for_line(source: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }
    let mut current = 0u32;
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            current += 1;
            if current == line {
                return Some(i + 1);
            }
        }
    }
    None
}

fn utf16_len(s: &str) -> usize {
    s.chars().map(|c| c.len_utf16()).sum()
}

fn utf8_offset_in_line(line: &str, character: usize) -> usize {
    if character >= line.len() {
        return line.len();
    }
    // Character counts UTF-8 code units; snap down to a char boundary.
    let mut idx = character.min(line.len());
    while idx > 0 && !line.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn utf16_offset_in_line(line: &str, character: usize) -> usize {
    let mut units = 0usize;
    for (byte_idx, ch) in line.char_indices() {
        let w = ch.len_utf16();
        if units + w > character {
            return byte_idx;
        }
        units += w;
        if units == character {
            return byte_idx + ch.len_utf8();
        }
    }
    line.len()
}
