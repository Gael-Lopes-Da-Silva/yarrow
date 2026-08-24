use std::path::{Path, PathBuf};

use crate::compiler::{CompileError, Compiler, RunResult};
use crate::diagnostics::{ColorChoice, Diagnostic, DiagnosticBatch, SourceFile, Span, render};
use crate::interpreter::EvalContext;
use crate::parser::Parser;
use crate::parser::ast::{Program, StmtKind};
use crate::tokenizer::{Token, Tokenizer};

/// How a session turns a checked program into code or executes it.
///
/// `Check` and `Jit` landed in Stage 13a. `Interpret` runs via
/// [`Session::interpret_source`] (Stage 13b). `Object` is still Stage 13c.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// Full type / ownership / stack / region checks; no JIT install.
    Check,
    /// Cranelift in-process machine code (default for `run` / `compile`).
    #[default]
    Jit,
    /// Native relocatable object (AOT). Not implemented yet (Stage 13c).
    Object,
    /// Stack VM / AST interpreter (`Session::interpret_source`).
    Interpret,
}

/// Options for one compile/check session.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Source path shown in diagnostics and used for module-relative imports.
    pub source_path: String,
    /// Extra module lookup roots (`"a.b"` => `a/b.yar`).
    pub module_search_paths: Vec<PathBuf>,
    /// Whether this session should require a top-level `main`.
    pub require_main: bool,
    /// Maximum number of diagnostics to collect before aborting.
    pub error_limit: usize,
    /// Backend / check mode for this session.
    pub mode: ExecutionMode,
}

impl CompileOptions {
    pub fn new(source_path: impl Into<String>) -> Self {
        Self {
            source_path: source_path.into(),
            module_search_paths: Vec::new(),
            require_main: true,
            error_limit: crate::diagnostics::DEFAULT_ERROR_LIMIT,
            mode: ExecutionMode::Jit,
        }
    }
}

/// Stateful frontend entry point: tokenize -> parse -> check / compile.
#[derive(Debug, Clone)]
pub struct Session {
    pub options: CompileOptions,
}

/// Program that passed semantic analysis (Stage 13a handoff for later backends).
#[derive(Debug, Clone)]
pub struct CheckedProgram {
    pub file: SourceFile,
    pub program: Program,
}

/// Successful JIT session artifact ready for run / IR dump.
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

    /// Tokenize source text without parsing or compiling.
    pub fn tokenize_source(
        &self,
        source: String,
    ) -> Result<(SourceFile, Vec<Token>), SessionDiagnostics> {
        let path = self.options.source_path.clone();
        let file = SourceFile::new(path.clone(), source.clone());
        match Tokenizer::new(source).tokenize() {
            Ok(tokens) => Ok((file, tokens)),
            Err(e) => Err(SessionDiagnostics {
                file,
                batch: tokenize_batch(e, &path, self.options.error_limit),
            }),
        }
    }

    /// Parse source text without compiling.
    pub fn parse_source(
        &self,
        source: String,
    ) -> Result<(SourceFile, Program), SessionDiagnostics> {
        let (file, tokens) = self.tokenize_source(source)?;
        match Parser::with_error_limit(tokens, self.options.error_limit).parse() {
            Ok(program) => Ok((file, program)),
            Err(batch) => Err(SessionDiagnostics { file, batch }),
        }
    }

    /// Type-check / ownership-check source without installing JIT code.
    ///
    /// Uses the same semantic pipeline as JIT compile, but skips
    /// `define_function` / module finalize (`ExecutionMode::Check`).
    pub fn check_source(&self, source: String) -> Result<CheckedProgram, SessionDiagnostics> {
        let (file, program) = self.parse_source(source)?;
        self.require_main_if_needed(&file, &program)?;
        let _compiler = self.lower(&file, &program, /* check_only */ true)?;
        Ok(CheckedProgram { file, program })
    }

    /// Compile source according to [`CompileOptions::mode`].
    ///
    /// - [`ExecutionMode::Jit`]: full check + JIT install (does not run `main`).
    /// - [`ExecutionMode::Check`]: same as [`Self::check_source`] but returns an
    ///   artifact whose compiler is check-only (`run_main` will fail).
    /// - [`ExecutionMode::Object`]: clear E391 until Stage 13c.
    /// - [`ExecutionMode::Interpret`]: clear error; use [`Self::interpret_source`].
    pub fn compile_source(&self, source: String) -> Result<SessionArtifact, SessionDiagnostics> {
        match self.options.mode {
            ExecutionMode::Object => {
                return Err(self.backend_not_ready(
                    source,
                    "E391",
                    "object codegen is not implemented yet",
                    "use --target jit for now, or wait for Stage 13c",
                ));
            }
            ExecutionMode::Interpret => {
                return Err(self.backend_not_ready(
                    source,
                    "E392",
                    "ExecutionMode::Interpret does not produce a JIT artifact",
                    "call Session::interpret_source to check and run on the interpreter",
                ));
            }
            ExecutionMode::Check | ExecutionMode::Jit => {}
        }

        let (file, program) = self.parse_source(source)?;
        self.require_main_if_needed(&file, &program)?;
        let check_only = matches!(self.options.mode, ExecutionMode::Check);
        let compiler = self.lower(&file, &program, check_only)?;
        Ok(SessionArtifact { file, compiler })
    }

    /// Check source, then execute `main` on the AST interpreter (Stage 13b).
    ///
    /// Returns the same [`RunResult`] shape as JIT `run_main` when supported.
    pub fn interpret_source(&self, source: String) -> Result<RunResult, SessionDiagnostics> {
        let checked = self.check_source(source)?;
        let mut ctx = EvalContext::new();
        if let Some(dir) = Path::new(&self.options.source_path).parent()
            && !dir.as_os_str().is_empty()
        {
            ctx.add_module_search_path(dir);
        }
        for p in &self.options.module_search_paths {
            ctx.add_module_search_path(p.clone());
        }
        if let Err(e) = ctx.load_program(&checked.program) {
            return Err(SessionDiagnostics {
                file: checked.file,
                batch: one_compile_error(e.into_compile_error(), self.options.error_limit),
            });
        }
        match ctx.run_main() {
            Ok(result) => Ok(result),
            Err(e) => Err(SessionDiagnostics {
                file: checked.file,
                batch: one_compile_error(e.into_compile_error(), self.options.error_limit),
            }),
        }
    }

    fn backend_not_ready(
        &self,
        source: String,
        code: &str,
        message: &str,
        help: &str,
    ) -> SessionDiagnostics {
        let path = self.options.source_path.clone();
        let file = SourceFile::new(path.clone(), source);
        let mut batch = DiagnosticBatch::with_limit(self.options.error_limit);
        let diag = Diagnostic::error(code, message)
            .with_path(path)
            .with_primary(Span::default(), "")
            .with_help(help);
        batch.push(diag);
        SessionDiagnostics { file, batch }
    }

    fn require_main_if_needed(
        &self,
        file: &SourceFile,
        program: &Program,
    ) -> Result<(), SessionDiagnostics> {
        if !self.options.require_main || has_main(program) {
            return Ok(());
        }
        let path = self.options.source_path.clone();
        let mut batch = DiagnosticBatch::with_limit(self.options.error_limit);
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
            .with_path(path)
            .with_primary(span, "")
            .with_note("running a `.yar` file requires a top-level `main` entry point")
            .with_help(
                "add `main function do ... end`, optionally `with T` for a printable result",
            );
        batch.push(diag);
        Err(SessionDiagnostics {
            file: file.clone(),
            batch,
        })
    }

    fn lower(
        &self,
        file: &SourceFile,
        program: &Program,
        check_only: bool,
    ) -> Result<Compiler, SessionDiagnostics> {
        let path = self.options.source_path.clone();
        let mut compiler = match Compiler::new() {
            Ok(compiler) => compiler,
            Err(e) => {
                return Err(SessionDiagnostics {
                    file: file.clone(),
                    batch: one_compile_error(e, self.options.error_limit),
                });
            }
        };
        compiler.set_error_limit(self.options.error_limit);
        compiler.set_source_path(path);
        compiler.set_check_only(check_only);
        if let Some(dir) = Path::new(&self.options.source_path).parent()
            && !dir.as_os_str().is_empty()
        {
            compiler.add_module_search_path(dir);
        }
        for p in &self.options.module_search_paths {
            compiler.add_module_search_path(p.clone());
        }

        if let Err(batch) = compiler.compile(program) {
            return Err(SessionDiagnostics {
                file: file.clone(),
                batch,
            });
        }

        Ok(compiler)
    }
}

impl SessionArtifact {
    pub fn run_main(&mut self) -> Result<RunResult, CompileError> {
        self.compiler.run_main()
    }

    /// Cranelift IR captured during compile (see `Compiler::emit_ir`).
    pub fn emit_ir(&self) -> String {
        self.compiler.emit_ir()
    }
}

fn has_main(program: &crate::parser::ast::Program) -> bool {
    program
        .items
        .iter()
        .any(|item| matches!(&item.kind, StmtKind::Function(f) if f.name == "main"))
}

fn tokenize_batch(
    err: crate::tokenizer::TokenizeError,
    path: &str,
    error_limit: usize,
) -> DiagnosticBatch {
    let mut batch = DiagnosticBatch::with_limit(error_limit);
    let diag = Diagnostic::error(err.code, err.message)
        .with_path(path)
        .with_primary(Span::from_location(err.location), "");
    batch.push(diag);
    batch
}

fn one_compile_error(err: CompileError, error_limit: usize) -> DiagnosticBatch {
    let mut batch = DiagnosticBatch::with_limit(error_limit);
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
