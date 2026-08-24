use std::path::{Path, PathBuf};

use crate::compiler::{CompileError, Compiler, RunResult};
use crate::diagnostics::{ColorChoice, Diagnostic, DiagnosticBatch, SourceFile, Span, render};
use crate::parser::Parser;
use crate::parser::ast::StmtKind;
use crate::tokenizer::Tokenizer;

/// Options for one compile/check session.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Source path shown in diagnostics and used for module-relative imports.
    pub source_path: String,
    /// Extra module lookup roots (`"a.b"` => `a/b.yar`).
    pub module_search_paths: Vec<PathBuf>,
    /// Whether this session should require a top-level `main`.
    pub require_main: bool,
}

impl CompileOptions {
    pub fn new(source_path: impl Into<String>) -> Self {
        Self {
            source_path: source_path.into(),
            module_search_paths: Vec::new(),
            require_main: true,
        }
    }
}

/// Stateful frontend entry point: tokenize -> parse -> compile.
#[derive(Debug, Clone)]
pub struct Session {
    pub options: CompileOptions,
}

/// Successful session artifact ready for run/check consumers.
pub struct SessionArtifact {
    pub file: SourceFile,
    pub compiler: Compiler,
}

/// Diagnostics emitted while tokenizing/parsing/compiling one source file.
pub struct SessionDiagnostics {
    pub file: SourceFile,
    pub batch: DiagnosticBatch,
}

impl Session {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    /// Compile source text. This performs full checking and code generation,
    /// but does not execute `main`.
    pub fn compile_source(&self, source: String) -> Result<SessionArtifact, SessionDiagnostics> {
        let path = self.options.source_path.clone();
        let file = SourceFile::new(path.clone(), source.clone());

        let tokens = match Tokenizer::new(source).tokenize() {
            Ok(tokens) => tokens,
            Err(e) => {
                return Err(SessionDiagnostics {
                    file,
                    batch: tokenize_batch(e, &path),
                });
            }
        };

        let program = match Parser::new(tokens).parse() {
            Ok(program) => program,
            Err(batch) => {
                return Err(SessionDiagnostics { file, batch });
            }
        };

        if self.options.require_main && !has_main(&program) {
            let mut batch = DiagnosticBatch::new();
            let span = program
                .items
                .iter()
                .find_map(|item| match &item.kind {
                    StmtKind::Function(_) => Some(item.span),
                    _ => None,
                })
                .or_else(|| program.items.first().map(|item| item.span))
                .unwrap_or_default();
            let diag = Diagnostic::error("E360", "program has no 'main' function")
                .with_path(path.clone())
                .with_primary(span, "")
                .with_note("running a `.yar` file requires a top-level `main` entry point")
                .with_help(
                    "add `main function do ... end`, optionally `with T` for a printable result",
                );
            batch.push(diag);
            return Err(SessionDiagnostics { file, batch });
        }

        let mut compiler = match Compiler::new() {
            Ok(compiler) => compiler,
            Err(e) => {
                return Err(SessionDiagnostics {
                    file,
                    batch: one_compile_error(e),
                });
            }
        };
        compiler.set_source_path(path);
        if let Some(dir) = Path::new(&self.options.source_path).parent()
            && !dir.as_os_str().is_empty()
        {
            compiler.add_module_search_path(dir);
        }
        for p in &self.options.module_search_paths {
            compiler.add_module_search_path(p.clone());
        }

        if let Err(batch) = compiler.compile(&program) {
            return Err(SessionDiagnostics { file, batch });
        }

        Ok(SessionArtifact { file, compiler })
    }
}

impl SessionArtifact {
    pub fn run_main(&mut self) -> Result<RunResult, CompileError> {
        self.compiler.run_main()
    }
}

fn has_main(program: &crate::parser::ast::Program) -> bool {
    program
        .items
        .iter()
        .any(|item| matches!(&item.kind, StmtKind::Function(f) if f.name == "main"))
}

fn tokenize_batch(err: crate::tokenizer::TokenizeError, path: &str) -> DiagnosticBatch {
    let mut batch = DiagnosticBatch::new();
    let diag = Diagnostic::error(err.code, err.message)
        .with_path(path)
        .with_primary(Span::from_location(err.location), "");
    batch.push(diag);
    batch
}

fn one_compile_error(err: CompileError) -> DiagnosticBatch {
    let mut batch = DiagnosticBatch::new();
    batch.push((*err.diagnostic).clone());
    batch
}

/// Render all diagnostics from one batch for one source file.
pub fn render_batch(batch: &DiagnosticBatch, file: &SourceFile, color: ColorChoice) -> String {
    let mut out = String::new();
    for diag in batch.iter() {
        let mut diag = diag.clone();
        if diag.path.is_empty() {
            diag.path = file.path.clone();
        }
        out.push_str(&render(&diag, file, color));
    }
    if batch.is_at_limit() {
        out.push_str(&format!(
            "error: aborting due to {} previous errors (limit {})\n",
            batch.len(),
            batch.limit()
        ));
    }
    out
}
