use std::path::{Path, PathBuf};

use crate::compiler::{CompileError, Compiler, RunResult};
use crate::diagnostics::{ColorChoice, Diagnostic, DiagnosticBatch, SourceFile, Span, render};
use crate::interpreter::EvalContext;
use crate::parser::Parser;
use crate::parser::ast::{Program, StmtKind};
use crate::tokenizer::{Token, Tokenizer};

/// How a session turns a checked program into code or executes it.
///
/// `Check` / `Jit` (13a), `Interpret` (13b), and `Object` emit (13c) are landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// Full type / ownership / stack / region checks; no JIT install.
    Check,
    /// Cranelift in-process machine code (default for `run` / `compile`).
    #[default]
    Jit,
    /// Native relocatable object (AOT). Use [`Session::compile_object_source`].
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
    /// Whether this session should require a top-level entry function.
    pub require_main: bool,
    /// Top-level entry function name (default [`crate::DEFAULT_ENTRY_NAME`]).
    ///
    /// Used by require-entry (E360), JIT / interpret `run_main`, and object
    /// emit (CRT trampoline target). CLI `--main` maps here; core does not
    /// parse argv.
    pub entry_name: String,
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
            entry_name: crate::DEFAULT_ENTRY_NAME.to_string(),
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
///
/// Warnings (Stage 20) may be non-empty while this remains `Ok` from
/// [`Session::check_source`].
#[derive(Debug, Clone)]
pub struct CheckedProgram {
    pub file: SourceFile,
    pub program: Program,
    pub warnings: DiagnosticBatch,
}

/// Successful JIT session artifact ready for run / IR dump.
pub struct SessionArtifact {
    pub file: SourceFile,
    pub compiler: Compiler,
}

/// Relocatable native object produced by [`Session::compile_object_source`].
///
/// Bytes are host ELF / Mach-O / COFF. Host runtime symbols (`print_str`, …)
/// remain unresolved imports until linked with [`crate::linkable_archive`].
/// Process entry is exported as [`crate::PROCESS_MAIN_SYMBOL`] (`main`).
pub struct ObjectArtifact {
    pub file: SourceFile,
    /// Object file bytes (non-empty on success).
    pub bytes: Vec<u8>,
    /// Cranelift IR captured during the same lower pass (debug / dump).
    pub ir: String,
    /// Yarrow entry name that process `main` calls in this object.
    pub entry_name: String,
}

/// Host executable produced by [`Session::compile_executable_source`].
///
/// Linked from the program object (with process `main`) and
/// [`crate::linkable_archive`] via a system linker (`ld` / `lld`), not `cc`.
pub struct ExecutableArtifact {
    pub file: SourceFile,
    /// Executable file bytes (non-empty on success).
    pub bytes: Vec<u8>,
    /// Cranelift IR from the object lower pass (dump parity).
    pub ir: String,
    /// Yarrow entry name that process `main` calls.
    pub entry_name: String,
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
    /// On success, [`CheckedProgram::warnings`] may contain Stage 20 warnings;
    /// they do not turn the result into `Err`.
    pub fn check_source(&self, source: String) -> Result<CheckedProgram, SessionDiagnostics> {
        let (file, program) = self.parse_source(source)?;
        self.require_main_if_needed(&file, &program)?;
        let mut compiler = self.lower(&file, &program, LowerKind::Jit { check_only: true })?;
        let warnings = compiler.take_warnings();
        Ok(CheckedProgram {
            file,
            program,
            warnings,
        })
    }

    /// Compile source according to [`CompileOptions::mode`].
    ///
    /// - [`ExecutionMode::Jit`]: full check + JIT install (does not run `main`).
    /// - [`ExecutionMode::Check`]: same as [`Self::check_source`] but returns an
    ///   artifact whose compiler is check-only (`run_main` will fail).
    /// - [`ExecutionMode::Object`]: clear error; use [`Self::compile_object_source`].
    /// - [`ExecutionMode::Interpret`]: clear error; use [`Self::interpret_source`].
    pub fn compile_source(&self, source: String) -> Result<SessionArtifact, SessionDiagnostics> {
        match self.options.mode {
            ExecutionMode::Object => {
                return Err(self.backend_not_ready(
                    source,
                    "E391",
                    "ExecutionMode::Object does not produce a JIT artifact",
                    "call Session::compile_object_source to emit a relocatable object",
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
        let compiler = self.lower(&file, &program, LowerKind::Jit { check_only })?;
        Ok(SessionArtifact { file, compiler })
    }

    /// Check + lower to a relocatable native object (Stage 13c).
    ///
    /// Ignores [`CompileOptions::mode`] other than using the same search paths /
    /// `require_main` / error limit. Prefer setting `mode` to
    /// [`ExecutionMode::Object`] for clarity.
    pub fn compile_object_source(
        &self,
        source: String,
    ) -> Result<ObjectArtifact, SessionDiagnostics> {
        let (file, program) = self.parse_source(source)?;
        self.require_main_if_needed(&file, &program)?;
        let module_name = object_module_name(&self.options.source_path);
        let compiler = self.lower(&file, &program, LowerKind::Object { module_name })?;
        let ir = compiler.emit_ir();
        let bytes = match compiler.emit_object() {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(SessionDiagnostics {
                    file,
                    batch: one_compile_error(e, self.options.error_limit),
                });
            }
        };
        if bytes.is_empty() {
            return Err(SessionDiagnostics {
                file,
                batch: one_compile_error(
                    CompileError::new(
                        "object emit produced an empty artifact",
                        Span::default(),
                        "E391",
                    ),
                    self.options.error_limit,
                ),
            });
        }
        Ok(ObjectArtifact {
            file,
            bytes,
            ir,
            entry_name: self.options.entry_name.clone(),
        })
    }

    /// Check, emit a program object (with process `main`), and link with the
    /// host runtime archive into a runnable executable (Stage 19).
    ///
    /// Uses a system linker (`ld` / `lld`). Does not invoke `cc` / `gcc` /
    /// `clang` as a compile or link driver, and never falls back to JIT.
    pub fn compile_executable_source(
        &self,
        source: String,
    ) -> Result<ExecutableArtifact, SessionDiagnostics> {
        let object = self.compile_object_source(source)?;
        let archive = match crate::linkable_archive() {
            Ok(archive) => archive,
            Err(msg) => {
                return Err(SessionDiagnostics {
                    file: object.file,
                    batch: crate::link::LinkError::new(
                        "E396",
                        format!("runtime archive unavailable: {msg}"),
                    )
                    .with_help(
                        "rebuild yarrow-core so `YARROW_RUNTIME_AOT_ARCHIVE` points at libyarrow_runtime_aot",
                    )
                    .into_batch(self.options.error_limit),
                });
            }
        };
        let bytes = match crate::link::link_executable(&object.bytes, &archive.bytes) {
            Ok(bytes) => bytes,
            Err(err) => {
                return Err(SessionDiagnostics {
                    file: object.file,
                    batch: err.into_batch(self.options.error_limit),
                });
            }
        };
        Ok(ExecutableArtifact {
            file: object.file,
            bytes,
            ir: object.ir,
            entry_name: object.entry_name,
        })
    }

    /// Check source, then execute the configured entry on the AST interpreter
    /// (Stage 13b).
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
        match ctx.run_entry(&self.options.entry_name) {
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
        let entry = self.options.entry_name.as_str();
        if !self.options.require_main || program.has_entry(entry) {
            return Ok(());
        }
        let path = self.options.source_path.clone();
        let mut batch = DiagnosticBatch::with_limit(self.options.error_limit);
        let span = program
            .entry_function(entry)
            .map(|(_, span)| span)
            .or_else(|| {
                program.items.iter().find_map(|item| match &item.kind {
                    StmtKind::Function(_) => Some(item.span),
                    _ => None,
                })
            })
            .or_else(|| program.items.first().map(|item| item.span))
            .unwrap_or_default();
        let diag = Diagnostic::error("E360", format!("program has no '{entry}' function"))
            .with_path(path)
            .with_primary(span, "")
            .with_note(format!(
                "running a `.yar` file requires a top-level `{entry}` entry point"
            ))
            .with_help(format!(
                "add `{entry} function do ... end`, optionally `with T` for a printable result"
            ));
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
        kind: LowerKind,
    ) -> Result<Compiler, SessionDiagnostics> {
        let path = self.options.source_path.clone();
        let mut compiler = match &kind {
            LowerKind::Jit { .. } => Compiler::new(),
            LowerKind::Object { module_name } => Compiler::new_object(module_name),
        }
        .map_err(|e| SessionDiagnostics {
            file: file.clone(),
            batch: one_compile_error(e, self.options.error_limit),
        })?;
        compiler.set_error_limit(self.options.error_limit);
        compiler.set_source_path(path);
        compiler.set_entry_name(self.options.entry_name.clone());
        if let LowerKind::Jit { check_only } = kind {
            compiler.set_check_only(check_only);
        }
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

enum LowerKind {
    Jit { check_only: bool },
    Object { module_name: String },
}

fn object_module_name(source_path: &str) -> String {
    Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("yarrow")
        .to_string()
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
            batch.error_count(),
            batch.limit()
        ));
    }
    out
}
