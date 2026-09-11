pub mod analysis;
pub mod compiler;
pub mod diagnostics;
pub mod entry;
pub mod interpreter;
pub mod link;
pub mod parser;
pub mod project;
pub mod runtime;
pub mod session;
pub mod target;
pub mod tokenizer;

pub use analysis::{TypeIndex, TypeProbe};
pub use compiler::{CompileError, Compiler, RunResult};
pub use diagnostics::{
    ColorChoice, DEFAULT_ERROR_LIMIT, Diagnostic, DiagnosticBatch, ExplainEntry, Severity,
    SourceFile, Span, explain_code, format_explain, normalize_code, render,
};
pub use entry::{DEFAULT_ENTRY_NAME, PROCESS_MAIN_SYMBOL};
pub use interpreter::{EvalContext, InterpretError, Interpreter, Value as InterpretValue};
pub use link::link_executable;
pub use parser::ParseError;
pub use parser::Parser;
pub use parser::ast::Program;
pub use parser::ast::Stmt;
pub use project::{
    CheckedProject, ModuleGraph, ModuleGraphEdge, ProjectOptions, ProjectRoot, check_project,
};
pub use runtime::{RuntimeArchive, link_symbol_names, linkable_archive, linkable_archive_for};
pub use session::{
    CheckedProgram, CompileOptions, ExecutableArtifact, ExecutionMode, ObjectArtifact, OptLevel,
    Session, SessionArtifact, SessionDiagnostics, render_batch,
};
pub use target::{TargetError, TargetTriple, supported_triple_names, supported_triples};
pub use tokenizer::Token;
pub use tokenizer::TokenKind;
pub use tokenizer::Tokenizer;
