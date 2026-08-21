use crate::diagnostics::Span;
use crate::tokenizer::token_kind::TokenKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    /// Start of the token (line / column / byte offset).
    pub location: Location,
    /// Exclusive end byte offset into the source.
    pub end_offset: usize,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: String, location: Location, end_offset: usize) -> Self {
        Self {
            kind,
            lexeme,
            location,
            end_offset: end_offset.max(location.offset),
        }
    }

    pub fn eof(location: Location) -> Self {
        Self::new(TokenKind::Eof, String::new(), location, location.offset)
    }

    pub fn span(&self) -> Span {
        Span::from_range(self.location, self.end_offset)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Location {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }
}

impl Default for Location {
    fn default() -> Self {
        Self::new(1, 1, 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizeError {
    pub message: String,
    pub location: Location,
    pub code: String,
}

impl TokenizeError {
    pub fn new(message: String, location: Location, code: String) -> Self {
        Self {
            message,
            location,
            code,
        }
    }
}

impl std::fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} at line {}, column {}",
            self.code, self.message, self.location.line, self.location.column
        )
    }
}

impl std::error::Error for TokenizeError {}
