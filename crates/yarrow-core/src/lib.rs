pub mod compiler;
pub mod diagnostics;
pub mod parser;
pub mod runtime;
pub mod session;
pub mod tokenizer;

pub use compiler::{CompileError, Compiler, RunResult};
pub use diagnostics::{
    ColorChoice, DEFAULT_ERROR_LIMIT, Diagnostic, DiagnosticBatch, ExplainEntry, SourceFile, Span,
    explain_code, format_explain, normalize_code, render,
};
pub use parser::ParseError;
pub use parser::Parser;
pub use parser::ast::Program;
pub use parser::ast::Stmt;
pub use session::{
    CheckedProgram, CompileOptions, ExecutionMode, Session, SessionArtifact, SessionDiagnostics,
    render_batch,
};
pub use tokenizer::Token;
pub use tokenizer::TokenKind;
pub use tokenizer::Tokenizer;
