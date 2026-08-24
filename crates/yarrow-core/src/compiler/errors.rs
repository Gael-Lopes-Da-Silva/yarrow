//! Compiler error type (wraps a rustc-style [`Diagnostic`]).

use crate::diagnostics::{Diagnostic, DiagnosticBatch, Label, Span};
use crate::tokenizer::token::Location;

/// An error produced while lowering a parsed `Program` to Cranelift IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub diagnostic: Box<Diagnostic>,
}

impl CompileError {
    pub fn new(message: impl Into<String>, span: Span, code: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            diagnostic: Box::new(Diagnostic::error(code, message).with_primary(span, "")),
        }
    }

    /// Build from a location (treated as a point span). Prefer [`Self::new`] with a real span.
    pub fn at(message: impl Into<String>, location: Location, code: impl Into<String>) -> Self {
        Self::new(message, Span::from_location(location), code)
    }

    pub fn unsupported(message: impl Into<String>, span: Span, code: impl Into<String>) -> Self {
        Self::new(message, span, code)
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.diagnostic.path = path.into();
        self
    }

    pub fn with_primary_message(mut self, message: impl Into<String>) -> Self {
        if let Some(label) = self.diagnostic.labels.iter_mut().find(|l| l.primary) {
            label.message = message.into();
        }
        self
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.diagnostic.labels.push(Label::secondary(span, message));
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.diagnostic.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic.helps.push(help.into());
        self
    }

    pub fn span(&self) -> Span {
        self.diagnostic.primary_span().unwrap_or_default()
    }

    pub fn code(&self) -> &str {
        &self.diagnostic.code
    }

    pub fn message(&self) -> &str {
        &self.diagnostic.message
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let loc = self.diagnostic.location();
        write!(
            f,
            "[{}] {} at line {}, column {}",
            self.code(),
            self.message(),
            loc.line,
            loc.column
        )
    }
}

impl std::error::Error for CompileError {}

impl From<crate::parser::ParseError> for CompileError {
    fn from(e: crate::parser::ParseError) -> Self {
        CompileError {
            diagnostic: Box::new(e.into_diagnostic()),
        }
    }
}

impl From<DiagnosticBatch> for CompileError {
    fn from(batch: DiagnosticBatch) -> Self {
        let mut items = batch.into_diagnostics();
        let diag = if items.is_empty() {
            Diagnostic::error("E200", "parse failed")
        } else {
            items.remove(0)
        };
        CompileError {
            diagnostic: Box::new(diag),
        }
    }
}

impl From<crate::tokenizer::token::TokenizeError> for CompileError {
    fn from(e: crate::tokenizer::token::TokenizeError) -> Self {
        CompileError::at(e.message, e.location, e.code)
    }
}

impl From<cranelift_module::ModuleError> for CompileError {
    fn from(e: cranelift_module::ModuleError) -> Self {
        CompileError::new(
            format!("cranelift module error: {e:?}"),
            Span::default(),
            "E399",
        )
    }
}
