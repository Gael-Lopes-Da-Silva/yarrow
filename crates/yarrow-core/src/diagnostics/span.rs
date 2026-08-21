//! Byte-offset spans into a source file.

use crate::tokenizer::token::Location;

/// Inclusive-exclusive byte range `[lo, hi)` into a [`crate::diagnostics::SourceFile`].
///
/// `line` / `column` cache the start position so messages remain useful without a
/// source map (1-based, matching the tokenizer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub lo: usize,
    pub hi: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(lo: usize, hi: usize) -> Self {
        Self {
            lo,
            hi: hi.max(lo),
            line: 1,
            column: 1,
        }
    }

    pub fn at(lo: usize, hi: usize, line: usize, column: usize) -> Self {
        Self {
            lo,
            hi: hi.max(lo),
            line: line.max(1),
            column: column.max(1),
        }
    }

    /// A one-byte caret at `offset` (or empty at EOF).
    pub fn point(offset: usize) -> Self {
        Self::new(offset, offset.saturating_add(1))
    }

    pub fn from_location(loc: Location) -> Self {
        Self::at(
            loc.offset,
            loc.offset.saturating_add(1),
            loc.line,
            loc.column,
        )
    }

    pub fn from_range(start: Location, end_offset: usize) -> Self {
        Self::at(start.offset, end_offset, start.line, start.column)
    }

    pub fn merge(self, other: Self) -> Self {
        if other.lo < self.lo {
            Self::at(other.lo, self.hi.max(other.hi), other.line, other.column)
        } else {
            Self::at(self.lo, self.hi.max(other.hi), self.line, self.column)
        }
    }

    pub fn is_empty(self) -> bool {
        self.lo >= self.hi
    }

    pub fn len(self) -> usize {
        self.hi.saturating_sub(self.lo)
    }

    pub fn start_location(self) -> Location {
        Location::new(self.line, self.column, self.lo)
    }
}

impl Default for Span {
    fn default() -> Self {
        Self::at(0, 0, 1, 1)
    }
}

impl From<Location> for Span {
    fn from(loc: Location) -> Self {
        Self::from_location(loc)
    }
}
