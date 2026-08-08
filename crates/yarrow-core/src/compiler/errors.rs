//! Compiler error type.

use crate::tokenizer::token::Location;

/// An error produced while lowering a parsed `Program` to Cranelift IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
    pub location: Location,
    pub code: String,
}

impl CompileError {
    pub fn new(message: impl Into<String>, location: Location, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location,
            code: code.into(),
        }
    }

    /// Shorthand for an unsupported feature, attributed to the source location
    /// of the offending construct.
    pub fn unsupported(
        message: impl Into<String>,
        location: Location,
        code: impl Into<String>,
    ) -> Self {
        Self::new(message, location, code)
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} at line {}, column {}",
            self.code, self.message, self.location.line, self.location.column
        )
    }
}

impl std::error::Error for CompileError {}

impl From<crate::parser::ParseError> for CompileError {
    fn from(e: crate::parser::ParseError) -> Self {
        CompileError::new(e.message, e.location, e.code)
    }
}

impl From<crate::tokenizer::token::TokenizeError> for CompileError {
    fn from(e: crate::tokenizer::token::TokenizeError) -> Self {
        CompileError::new(e.message, e.location, e.code)
    }
}

impl From<cranelift_module::ModuleError> for CompileError {
    fn from(e: cranelift_module::ModuleError) -> Self {
        CompileError::new(
            format!("cranelift module error: {e:?}"),
            Location::default(),
            "E399",
        )
    }
}
